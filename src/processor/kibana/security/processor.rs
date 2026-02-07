// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::{DocumentExporter, KibanaMetadata, Lookups, Metadata};
use super::{Actions, Roles, Users};
use crate::{exporter::Exporter, processor::ProcessorSummary};
use serde::Serialize;
use serde_json::{Value, json};

impl DocumentExporter<Lookups, KibanaMetadata> for Roles {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "settings-kibana.security-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let docs = process_array(self.0, metadata_doc);

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana security roles: {}", err),
        }
        summary
    }
}

impl DocumentExporter<Lookups, KibanaMetadata> for Users {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "settings-kibana.security-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let docs = process_array(self.0, metadata_doc);

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana security users: {}", err),
        }
        summary
    }
}

impl DocumentExporter<Lookups, KibanaMetadata> for Actions {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "settings-kibana.security-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let docs = process_array(self.0, metadata_doc);

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana security actions: {}", err),
        }
        summary
    }
}

#[derive(Serialize)]
struct SecurityDoc {
    #[serde(flatten)]
    metadata: Value,
    #[serde(flatten)]
    data: Value,
}

fn process_array(data: Value, metadata: Value) -> Vec<Value> {
    match data {
        Value::Array(arr) => arr
            .into_iter()
            .map(|item| {
                json!(SecurityDoc {
                    metadata: metadata.clone(),
                    data: item
                })
            })
            .collect(),
        _ => vec![json!(SecurityDoc { metadata, data })],
    }
}
