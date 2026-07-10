// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

//! Kibana diagnostic processor
//!
//! This module provides the entry point for processing Kibana diagnostic bundles.
//! It follows a single-node workflow similar to Logstash, extracting metrics
//! and settings from various JSON files.

/// Kibana alerts processor
mod alerts;
/// Collector definition for Kibana diagnostics
mod collector;
/// Kibana detection engine processor
mod detection_engine;
/// Kibana fleet processor
mod fleet;
/// Kibana health processor
mod health;
/// Kibana diagnostic metadata
mod metadata;
/// Kibana node stats processor
mod node_stats;
/// Kibana security processor
mod security;
/// Kibana settings processor
mod settings;
/// Kibana spaces processor
mod spaces;
/// Kibana status processor
mod status;
/// Kibana synthetics and uptime processor
mod synthetics_uptime;
/// Kibana version
mod version;

pub use collector::KibanaCollector;

use super::{
    DiagnosticProcessor, DocumentExporter, Metadata, ProcessorSummary,
    api::ProcessSelection,
    diagnostic::{DataSource, DiagnosticManifest, DiagnosticReport, DiagnosticReportBuilder},
};
use crate::{
    data::{self, Product},
    exporter::Exporter,
    receiver::Receiver,
};
use alerts::{AlertHealth, Alerts};
use detection_engine::{DetectionEngineHealth, DetectionEngineRules};
use eyre::{Result, eyre};
use fleet::{AgentPolicies, AgentStatus, Agents, Packages};
use health::{StackMonitoringHealth, TaskManagerHealth};
use metadata::KibanaMetadata;
use node_stats::NodeStats;
use security::{Actions, Roles, Users};
use serde::{Serialize, de::DeserializeOwned};
use settings::{FleetSettings, UptimeSettings};
use spaces::Spaces;
use status::Status;
use std::sync::Arc;
use synthetics_uptime::{SyntheticsFilters, UptimeLocations};
use tokio::sync::mpsc;

#[derive(Serialize)]
pub struct KibanaDiagnostic {
    lookups: Lookups,
    metadata: KibanaMetadata,
    #[serde(skip)]
    exporter: Arc<Exporter>,
    #[serde(skip)]
    receiver: Arc<Receiver>,
}

#[derive(Serialize)]
struct Lookups {}

impl KibanaDiagnostic {
    pub fn uuid(&self) -> &str {
        &self.metadata.diagnostic.uuid
    }

    async fn process_datasource<T>(&mut self, summary_tx: mpsc::Sender<ProcessorSummary>) -> Result<()>
    where
        T: DataSource + DocumentExporter<Lookups, KibanaMetadata> + DeserializeOwned + Send + Sync,
    {
        match self.receiver.get::<T>().await {
            Ok(data) => {
                let summary = data
                    .documents_export(&self.exporter, &self.lookups, &self.metadata)
                    .await;
                summary_tx.send(summary).await.map_err(|err| {
                    tracing::error!("Failed to send summary: {}", err);
                    eyre!(err)
                })
            }
            Err(e) => {
                tracing::debug!("Skipping {}: {}", T::name(), e);
                Ok(())
            }
        }
    }
}

impl DiagnosticProcessor for KibanaDiagnostic {
    async fn try_new(
        receiver: Arc<Receiver>,
        exporter: Arc<Exporter>,
        manifest: DiagnosticManifest,
        _process_selection: Option<ProcessSelection>,
    ) -> Result<(Box<Self>, DiagnosticReport)> {
        let kibana_version = receiver.get::<version::Version>().await?;
        let metadata = KibanaMetadata::try_new(manifest, kibana_version)?;
        let report = DiagnosticReportBuilder::from(metadata.diagnostic.clone())
            .product(Product::Kibana)
            .receiver(receiver.to_string())
            .build()?;

        Ok((
            Box::new(Self {
                lookups: Lookups {},
                receiver,
                exporter,
                metadata,
            }),
            report,
        ))
    }

    async fn process(mut self, summary_tx: mpsc::Sender<ProcessorSummary>) -> Result<()> {
        tracing::debug!("Running Kibana diagnostic processors");
        if tracing::enabled!(tracing::Level::DEBUG) {
            data::save_file("diagnostic.json", &self)?;
        }

        // Core Processors
        self.process_datasource::<NodeStats>(summary_tx.clone()).await?;
        self.process_datasource::<Status>(summary_tx.clone()).await?;
        self.process_datasource::<FleetSettings>(summary_tx.clone()).await?;
        self.process_datasource::<UptimeSettings>(summary_tx.clone()).await?;

        // Domain Processors
        self.process_datasource::<Roles>(summary_tx.clone()).await?;
        self.process_datasource::<Users>(summary_tx.clone()).await?;
        self.process_datasource::<Actions>(summary_tx.clone()).await?;

        self.process_datasource::<Agents>(summary_tx.clone()).await?;
        self.process_datasource::<AgentPolicies>(summary_tx.clone()).await?;
        self.process_datasource::<Packages>(summary_tx.clone()).await?;
        self.process_datasource::<AgentStatus>(summary_tx.clone()).await?;

        self.process_datasource::<Alerts>(summary_tx.clone()).await?;
        self.process_datasource::<AlertHealth>(summary_tx.clone()).await?;

        self.process_datasource::<Spaces>(summary_tx.clone()).await?;

        self.process_datasource::<TaskManagerHealth>(summary_tx.clone()).await?;
        self.process_datasource::<StackMonitoringHealth>(summary_tx.clone())
            .await?;

        self.process_datasource::<SyntheticsFilters>(summary_tx.clone()).await?;
        self.process_datasource::<UptimeLocations>(summary_tx.clone()).await?;

        self.process_datasource::<DetectionEngineHealth>(summary_tx.clone())
            .await?;
        self.process_datasource::<DetectionEngineRules>(summary_tx.clone())
            .await?;

        Ok(())
    }

    fn id(&self) -> &str {
        &self.metadata.diagnostic.id
    }

    fn origin(&self) -> (String, String, String) {
        (
            self.metadata.node.name.clone(),
            self.metadata.node.id.clone(),
            "node".to_string(),
        )
    }
}
