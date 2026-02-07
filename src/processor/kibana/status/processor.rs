// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::{DocumentExporter, KibanaMetadata, Lookups, Metadata};
use super::Status;
use crate::{exporter::Exporter, processor::ProcessorSummary};
use serde::Serialize;
use serde_json::{Value, json};

impl DocumentExporter<Lookups, KibanaMetadata> for Status {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "metrics-kibana.status-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let status_doc = json!(StatusDoc::new(self, metadata_doc));

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, vec![status_doc]).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana status: {}", err),
        }
        summary
    }
}

#[derive(Serialize)]
struct StatusDoc {
    #[serde(flatten)]
    metadata: Value,
    #[serde(flatten)]
    status: Status,
}

impl StatusDoc {
    fn new(status: Status, metadata: Value) -> Self {
        Self { metadata, status }
    }
}
