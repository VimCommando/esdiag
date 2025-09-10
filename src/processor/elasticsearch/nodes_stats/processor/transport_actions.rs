// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::super::nodes::NodeDocument;
use super::{ElasticsearchMetadata, Metadata, ProcessorSummary};
use crate::exporter::Exporter;
use eyre::{OptionExt, Result};
use json_patch::merge;
use serde_json::{Value, json};

/// Extract transport.actions

pub async fn extract(
    exporter: &Exporter,
    summary: &mut ProcessorSummary,
    mut actions: Value,
    metadata: &ElasticsearchMetadata,
    node_metadata: Option<&NodeDocument>,
) -> Result<()> {
    let metadata = metadata
        .for_data_stream("metrics-node.transport.actions-esdiag")
        .as_meta_doc();

    let actions = actions
        .as_object_mut()
        .ok_or_eyre("Error extracting node transport.actions data")?;

    let mut docs = Vec::<Value>::with_capacity(100);
    docs.extend(
        actions
            .into_iter()
            .collect::<Vec<_>>()
            .drain(..)
            .map(|(name, action)| {
                let mut action = json!({
                    "node": node_metadata,
                    "transport": {
                        "action": action,
                    },
                });

                let action_patch = json!({
                    "transport": {
                        "action": {
                            "name": name,
                        },
                    },
                });

                merge(&mut action, &action_patch);
                merge(&mut action, &metadata);
                action
            }),
    );

    exporter.write(summary, &mut docs).await?;
    log::trace!("transport_actions: {}", summary.docs);
    Ok(())
}
