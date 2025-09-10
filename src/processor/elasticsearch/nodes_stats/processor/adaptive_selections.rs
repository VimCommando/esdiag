// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::super::{Lookup, nodes::NodeDocument};
use super::{ElasticsearchMetadata, Metadata, ProcessorSummary};
use crate::exporter::Exporter;
use eyre::Result;
use json_patch::merge;
use serde_json::{Value, json};

/// Extract adaptive_selection
pub async fn extract(
    exporter: &Exporter,
    summary: &mut ProcessorSummary,
    adaptive_selection: Option<Value>,
    metadata: &ElasticsearchMetadata,
    node_metadata: Option<&NodeDocument>,
    lookup_node: &Lookup<NodeDocument>,
) -> Result<()> {
    let adaptive_selection_metadata = metadata
        .for_data_stream("metrics-node.adaptive_selection-esdiag")
        .as_meta_doc();

    let adaptive_selection = match adaptive_selection {
        Some(Value::Object(data)) => data,
        _ => return Err(eyre::eyre!("Error extracting node.adaptive_selection data")),
    };

    let mut docs = Vec::<Value>::with_capacity(200);
    docs.extend(
        adaptive_selection
            .into_iter()
            .collect::<Vec<(String, Value)>>()
            .drain(..)
            .map(|(peer_node_id, adaptive_selection)| {
                let mut doc = json!({
                    "adaptive_selection": adaptive_selection,
                    "node": node_metadata,
                });

                let peer_node_patch = json!({
                    "adaptive_selection": {
                        "node": lookup_node.by_id(&peer_node_id),
                    },
                });

                merge(&mut doc, &peer_node_patch);
                merge(&mut doc, &adaptive_selection_metadata);
                doc
            }),
    );
    exporter.write(summary, &mut docs).await?;
    log::trace!("adaptive_selections: {}", summary.docs);
    Ok(())
}
