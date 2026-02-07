// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::{DocumentExporter, KibanaMetadata, Lookups, Metadata};
use super::Logs;
use crate::{exporter::Exporter, processor::ProcessorSummary};
use serde::Serialize;
use serde_json::{Value, json};

impl DocumentExporter<Lookups, KibanaMetadata> for Logs {
    async fn documents_export(
        self,
        exporter: &Exporter,
        lookups: &Lookups,
        metadata: &KibanaMetadata,
    ) -> ProcessorSummary {
        Self::export_raw(self.0, exporter, lookups, metadata).await
    }

    async fn export_raw(data: String, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "logs-kibana.diagnostics-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();

        let docs: Vec<Value> = data
            .lines()
            .map(|line| {
                json!(LogDoc {
                    metadata: metadata_doc.clone(),
                    message: line.to_string(),
                })
            })
            .collect();

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, docs).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana logs: {}", err),
        }
        summary
    }
}

#[derive(Serialize)]
struct LogDoc {
    #[serde(flatten)]
    metadata: Value,
    message: String,
}
