// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::super::nodes::NodeDocument;
use super::{ElasticsearchMetadata, Metadata, ProcessorSummary};
use crate::exporter::Exporter;
use eyre::{OptionExt, Result};
use json_patch::merge;
use serde_json::{Value, json};

/// Extract discovery.cluster_applier_stats.recordings dataset
pub async fn extract(
    exporter: &Exporter,
    summary: &mut ProcessorSummary,
    mut cluster_applier_stats: Value,
    metadata: &ElasticsearchMetadata,
    node_metadata: Option<&NodeDocument>,
) -> Result<()> {
    let metadata = metadata
        .for_data_stream("metrics-node.discovery.cluster_applier-esdiag")
        .as_meta_doc();
    let recordings = cluster_applier_stats["recordings"]
        .as_array_mut()
        .ok_or_eyre("Error extracting node.discovery.cluster_applier data")?;

    let mut docs = Vec::<Value>::with_capacity(200);

    docs.extend(recordings.drain(..).map(|recording| {
        let mut doc = json!({
            "cluster_applier_stats": recording,
            "node": node_metadata,
        });

        merge(&mut doc, &metadata);
        doc
    }));

    exporter.write(summary, &mut docs).await?;
    log::trace!("recordings: {}", summary.docs);
    Ok(())
}
