// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::super::nodes::NodeDocument;
use super::{ElasticsearchMetadata, Metadata};
use json_patch::merge;
use rayon::prelude::*;
use serde_json::{Value, json};

/// Extract discovery.cluster_applier_stats.recordings dataset
pub fn extract(
    cluster_applier_stats: Value,
    metadata: &ElasticsearchMetadata,
    node_summary: Option<&NodeDocument>,
) -> Vec<Value> {
    let metadata = metadata
        .for_data_stream("metrics-node.discovery.cluster_applier-esdiag")
        .as_meta_doc();

    let recordings: Vec<_> = match cluster_applier_stats["recordings"].as_array() {
        Some(recordings) => recordings
            .par_iter()
            .map(|recording| {
                let mut doc = json!({
                    "cluster_applier_stats": recording,
                    "node": node_summary,
                });

                merge(&mut doc, &metadata);
                doc
            })
            .collect(),
        None => Vec::new(),
    };

    log::trace!("recordings: {}", recordings.len());
    recordings
}
