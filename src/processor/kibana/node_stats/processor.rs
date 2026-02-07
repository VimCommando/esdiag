// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::{DocumentExporter, KibanaMetadata, Lookups, Metadata};
use super::NodeStats;
use crate::{exporter::Exporter, processor::ProcessorSummary};
use serde::Serialize;
use serde_json::{Value, json};

impl DocumentExporter<Lookups, KibanaMetadata> for NodeStats {
    async fn documents_export(self, exporter: &Exporter, _: &Lookups, metadata: &KibanaMetadata) -> ProcessorSummary {
        let data_stream = "metrics-kibana.node-esdiag".to_string();
        let metadata_doc = metadata.for_data_stream(&data_stream).as_meta_doc();
        let node_doc = json!(NodeStatsDoc::new(self, metadata_doc));

        let mut summary = ProcessorSummary::new(data_stream.clone());
        match exporter.send(data_stream, vec![node_doc]).await {
            Ok(batch) => summary.add_batch(batch),
            Err(err) => tracing::error!("Failed to send Kibana node stats: {}", err),
        }
        summary
    }
}

#[derive(Serialize)]
struct NodeStatsDoc {
    #[serde(flatten)]
    metadata: Value,
    node: Value,
}

impl NodeStatsDoc {
    fn new(node: NodeStats, metadata: Value) -> Self {
        let mut node_with_metadata = json!(metadata.get("node").take());
        json_patch::merge(&mut node_with_metadata, &json!(node));

        Self {
            metadata,
            node: node_with_metadata,
        }
    }
}
