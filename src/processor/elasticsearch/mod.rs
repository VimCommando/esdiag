// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

/// The `_alias` API
mod alias;
/// The `_cluster/settings` API
mod cluster_settings;
/// Collector definition for Elasticsearch diagnostics
pub mod collector;
/// The `_data_stream` API
mod data_stream;
/// The `_health_report` API
mod health_report;
/// The `_ilm/explain` API
mod ilm_explain;
/// The `_ilm/policy` API
mod ilm_policies;
/// The `_settings` API
mod indices_settings;
/// The `_stats` API
mod indices_stats;
/// The `_license` API
mod licenses;
/// Elasticsearch diagnostics metadata
mod metadata;
/// The `_nodes` API
mod nodes;
/// The `_nodes/stats` API
mod nodes_stats;
/// The `_pending_tasks` API
mod pending_tasks;
/// The `_searchable_snapshots_cache/stats` API
mod searchable_snapshots_cache_stats;
/// The `_searchable_snapshots/stats` API
mod searchable_snapshots_stats;
/// The `_slm/policy` API
mod slm_policies;
/// The `_tasks` API
mod tasks;
/// The cluster `/` API -- "You know, for search!"
mod version;

pub use metadata::{ElasticsearchMetadata, ElasticsearchVersion};
pub use {
    licenses::License,
    version::{Cluster, Version},
};

use super::{
    DataProcessor, DiagnosticProcessor, Metadata,
    diagnostic::{
        DataSource, DiagnosticManifest, DiagnosticReport, DiagnosticReportBuilder, Lookup, Product,
        report::ProcessorSummary,
    },
};
use crate::{
    data, exporter::Exporter, processor::elasticsearch::health_report::HealthReport,
    receiver::Receiver,
};
use eyre::{Result, eyre};
use futures::{future::join_all, stream::FuturesUnordered};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{pin::Pin, sync::Arc};
use tokio::{sync::RwLock, task::JoinHandle};
use {
    alias::{Alias, AliasList},
    cluster_settings::ClusterSettings,
    data_stream::{DataStream, DataStreams},
    ilm_explain::{IlmExplain, IlmStats},
    ilm_policies::IlmPolicies,
    indices_settings::{IndexSettings, IndicesSettings},
    indices_stats::IndicesStats,
    licenses::Licenses,
    nodes::{NodeDocument, Nodes},
    nodes_stats::NodesStats,
    pending_tasks::PendingTasks,
    searchable_snapshots_cache_stats::{SearchableSnapshotsCacheStats, SharedCacheStats},
    searchable_snapshots_stats::SearchableSnapshotsStats,
    slm_policies::SlmPolicies,
    tasks::Tasks,
};

type ExporterDocumentQueue = Arc<RwLock<Vec<(String, Vec<Value>)>>>;

#[derive(Clone, Serialize)]
pub struct ElasticsearchDiagnostic {
    lookups: Arc<Lookups>,
    metadata: Arc<ElasticsearchMetadata>,
    #[serde(skip)]
    exporter: Arc<Exporter>,
    #[serde(skip)]
    receiver: Arc<Receiver>,
    #[serde(skip)]
    report: Arc<RwLock<DiagnosticReport>>,
    #[serde(skip)]
    queue: ExporterDocumentQueue,
}

impl DiagnosticProcessor for ElasticsearchDiagnostic {
    async fn new(
        manifest: DiagnosticManifest,
        receiver: Receiver,
        exporter: Exporter,
    ) -> Result<Box<Self>> {
        let cluster = receiver.get::<version::Cluster>().await?;
        let display_name = receiver
            .get::<cluster_settings::ClusterSettings>()
            .await?
            .get_display_name();
        let metadata =
            ElasticsearchMetadata::try_new(manifest, cluster.with_display_name(display_name))?;
        let mut report = DiagnosticReportBuilder::from(metadata.diagnostic.clone())
            .product(Product::Elasticsearch)
            .receiver(receiver.to_string())
            .build()?;

        let lookups = Lookups {
            alias: Lookup::from(receiver.get::<AliasList>().await),
            data_stream: Lookup::from(receiver.get::<DataStreams>().await),
            index_settings: Lookup::from(receiver.get::<IndicesSettings>().await),
            node: Lookup::from(receiver.get::<Nodes>().await),
            ilm_explain: Lookup::from(receiver.get::<IlmExplain>().await),
            shared_cache: Lookup::from(receiver.get::<SearchableSnapshotsCacheStats>().await),
        };
        let license = receiver
            .get::<Licenses>()
            .await
            .map(|licenses| licenses.license)
            .ok();

        report.add_license(license);
        report.add_lookup("alias", &lookups.alias);
        report.add_lookup("data_stream", &lookups.data_stream);
        report.add_lookup("index_settings", &lookups.index_settings);
        report.add_lookup("node", &lookups.node);
        report.add_lookup("ilm_explain", &lookups.ilm_explain);
        report.add_lookup("shared_cache", &lookups.shared_cache);

        Ok(Box::new(Self {
            exporter: Arc::new(exporter),
            lookups: Arc::new(lookups),
            metadata: Arc::new(metadata.clone()),
            queue: Arc::new(RwLock::new(Vec::<(String, Vec<Value>)>::new())),
            receiver: Arc::new(receiver),
            report: Arc::new(RwLock::new(report)),
        }))
    }

    async fn run(self) -> Result<DiagnosticReport> {
        log::debug!("Running Elasticsearch diagnostic processors");
        if self.exporter.is_connected().await == false {
            return Err(eyre!("Exporter is not connected"));
        }

        if log::max_level() >= log::Level::Debug {
            data::save_file("diagnostic.json", &self)?;
        }

        let diag = Arc::new(self);

        let mut futures = FuturesUnordered::new();
        let tasks = vec![
            spawn_processor::<ClusterSettings>(diag.clone()),
            spawn_processor::<IndicesSettings>(diag.clone()),
            spawn_processor::<IndicesStats>(diag.clone()),
            spawn_processor::<HealthReport>(diag.clone()),
            spawn_processor::<Nodes>(diag.clone()),
            spawn_processor::<NodesStats>(diag.clone()),
            spawn_processor::<IlmPolicies>(diag.clone()),
            spawn_processor::<SlmPolicies>(diag.clone()),
            //spawn_processor::<SearchableSnapshotsStats>(diag.clone()),
            spawn_processor::<Tasks>(diag.clone()),
            spawn_processor::<PendingTasks>(diag.clone()),
        ];
        futures.extend(tasks);

        {
            let mut report = diag.report.write().await;
            report.add_identifiers(diag.exporter.identifiers());
            join_all(futures)
                .await
                .into_iter()
                .filter_map(Result::ok)
                .flatten()
                .for_each(|summary| report.add_processor_summary(summary));

            report.add_origin(
                Some(diag.metadata.cluster.display_name.clone()),
                Some(diag.metadata.cluster.uuid.clone()),
                Some("cluster".to_string()),
            );
            diag.exporter.save_report(&*report).await?;
        }

        // Clone the report after releasing the write lock
        let report = diag.report.read().await.clone();
        Ok(report)
    }

    fn id(&self) -> &str {
        &self.metadata.diagnostic.id
    }
}

type DataProcessorTask = Pin<Box<JoinHandle<Option<ProcessorSummary>>>>;

fn spawn_processor<T>(diagnostic: Arc<ElasticsearchDiagnostic>) -> DataProcessorTask
where
    T: DataSource + DataProcessor<Lookups, ElasticsearchMetadata> + DeserializeOwned + Send + Sync,
{
    let exporter = diagnostic.exporter.clone();
    let lookups = diagnostic.lookups.clone();
    let metadata = diagnostic.metadata.clone();
    let queue = diagnostic.queue.clone();
    let receiver = diagnostic.receiver.clone();

    Box::pin(tokio::task::spawn(async move {
        let docs = receiver
            .get::<T>()
            .await
            .map(|data| data.generate_docs(lookups, metadata));
        match docs {
            Ok(docs) => {
                queue.write().await.push(docs);
                // Process queue directly instead of calling diagnostic.process_queue
                let mut queue_guard = queue.write().await;
                if let Some((index, docs)) = queue_guard.pop() {
                    log::debug!("Processing queue {index}");
                    exporter
                        .write(index, docs)
                        .await
                        .ok()
                        .map(|summary| summary.rename(T::name()).was_parsed())
                } else {
                    log::warn!("Queue was empty");
                    None
                }
            }
            Err(e) => {
                log::warn!("No {} data found: {e}", T::name());
                Some(ProcessorSummary::new(T::name()))
            }
        }
    }))
}

#[derive(Serialize)]
pub struct Lookups {
    pub alias: Lookup<Alias>,
    pub data_stream: Lookup<DataStream>,
    pub ilm_explain: Lookup<IlmStats>,
    pub index_settings: Lookup<IndexSettings>,
    pub node: Lookup<NodeDocument>,
    pub shared_cache: Lookup<SharedCacheStats>,
}
