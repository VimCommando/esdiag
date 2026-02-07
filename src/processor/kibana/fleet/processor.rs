// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::{DocumentExporter, KibanaMetadata, Lookups, Metadata};
use super::{AgentPolicies, AgentStatus, Agents, Packages};
use crate::{exporter::Exporter, processor::ProcessorSummary};
use serde::Serialize;
use serde_json::{Value, json};

impl DocumentExporter<Lookups, KibanaMetadata> for Agents {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "settings-kibana.fleet-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let docs = process_fleet_data(self.0, metadata_doc);

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana fleet agents: {}", err),
        }
        summary
    }
}

impl DocumentExporter<Lookups, KibanaMetadata> for AgentPolicies {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "settings-kibana.fleet-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let docs = process_fleet_data(self.0, metadata_doc);

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana fleet agent policies: {}", err),
        }
        summary
    }
}

impl DocumentExporter<Lookups, KibanaMetadata> for Packages {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "settings-kibana.fleet-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let docs = process_fleet_data(self.0, metadata_doc);

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana fleet packages: {}", err),
        }
        summary
    }
}

impl DocumentExporter<Lookups, KibanaMetadata> for AgentStatus {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "metrics-kibana.fleet-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let doc = json!(FleetDoc {
            metadata: metadata_doc,
            data: self.0
        });

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, vec![doc]).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana fleet agent status: {}", err),
        }
        summary
    }
}

#[derive(Serialize)]
struct FleetDoc {
    #[serde(flatten)]
    metadata: Value,
    #[serde(flatten)]
    data: Value,
}

fn process_fleet_data(data: Value, metadata: Value) -> Vec<Value> {
    // Fleet APIs often return { "list": [...], "total": ..., ... }
    let items = if let Some(list) = data.get("list").and_then(|l| l.as_array()) {
        list.clone()
    } else if let Some(items) = data.get("items").and_then(|i| i.as_array()) {
        items.clone()
    } else if let Some(arr) = data.as_array() {
        arr.clone()
    } else {
        vec![data]
    };

    items
        .into_iter()
        .map(|item| {
            json!(FleetDoc {
                metadata: metadata.clone(),
                data: item
            })
        })
        .collect()
}
