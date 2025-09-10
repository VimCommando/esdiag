// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

mod adaptive_selections;
mod cluster_applier_stats;
mod http_clients;
mod ingest_pipelines;
mod transport_actions;

use super::super::super::{Exporter, ProcessorSummary};
use super::super::{DocumentExporter, ElasticsearchMetadata, Lookups, Metadata};
use super::NodesStats;
use json_patch::merge;
use serde_json::{Value, json};
use std::sync::LazyLock;

static INGEST_ROLE: LazyLock<String> = LazyLock::new(|| String::from("ingest"));

impl DocumentExporter<Lookups, ElasticsearchMetadata> for NodesStats {
    async fn documents_export(
        self,
        exporter: &Exporter,
        lookups: &Lookups,
        metadata: &ElasticsearchMetadata,
    ) -> ProcessorSummary {
        let nodes_stats = self.nodes;
        log::debug!("nodes: {}", nodes_stats.len());
        let data_stream = "metrics-node-esdiag".to_string();
        let node_stats_metadata = metadata.for_data_stream(&data_stream).as_meta_doc();
        let lookup_node = &lookups.node;
        let lookup_shared_cache = &lookups.shared_cache;
        let mut summary = ProcessorSummary::new(data_stream);

        let mut docs: Vec<Value> = Vec::with_capacity(200);
        for (node_id, mut node_stats) in nodes_stats {
            let node_metadata = lookup_node.by_id(&node_id);
            let allocated_processors = node_metadata
                .map(|node| node.os.allocated_processors)
                .unwrap_or(1);
            node_stats.calculate_stats(allocated_processors);

            match node_stats.transport {
                Some(ref mut transport) => {
                    if let Err(e) = transport_actions::extract(
                        exporter,
                        &mut summary,
                        transport["actions"].take(),
                        &metadata,
                        node_metadata,
                    )
                    .await
                    {
                        log::error!(
                            "Error extracting transport stats for node {}: {}",
                            node_id,
                            e
                        );
                    }
                }
                None => {
                    log::trace!("Skipping transport stats for node {}", node_id);
                }
            }

            if let Err(e) = http_clients::extract(
                exporter,
                &mut summary,
                node_stats.http["clients"].take(),
                &metadata,
                node_metadata,
            )
            .await
            {
                log::error!("Error extracting HTTP clients stats: {}", e);
            }

            if let Err(e) = adaptive_selections::extract(
                exporter,
                &mut summary,
                node_stats.adaptive_selection.take(),
                &metadata,
                node_metadata,
                lookup_node,
            )
            .await
            {
                log::error!("Error extracting adaptive selection stats: {}", e);
            }

            if let Err(e) = cluster_applier_stats::extract(
                exporter,
                &mut summary,
                node_stats.discovery["cluster_applier_stats"].take(),
                &metadata,
                node_metadata,
            )
            .await
            {
                log::error!("Error extracting cluster applier stats: {}", e);
            }

            let ingest_pipelines_docs = match node_stats.roles.contains(&*INGEST_ROLE) {
                true => ingest_pipelines::extract(
                    node_stats.ingest.pipelines.take(),
                    &metadata,
                    node_metadata,
                ),
                false => Vec::new(),
            };

            // Final node_stats document
            let mut doc = json!({
                "node": &node_stats,
                "shared_cache": lookup_shared_cache.by_id(node_id.as_str()),
            });

            let omit_patch = json!({
                "node" : {
                    "http": { "routes": null },
                }
            });

            let node_summary_patch = json!({"node": node_metadata});

            merge(&mut doc, &node_stats_metadata);
            merge(&mut doc, &node_summary_patch);
            merge(&mut doc, &omit_patch);

            // Start a vec with the top-level node_stats doc
            docs.push(doc);
            //docs.extend(adaptive_selection_docs);
            //docs.extend(http_clients_docs);
            docs.extend(ingest_pipelines_docs);
            //docs.extend(recording_docs);
            //docs.extend(transport_actions_docs);
            log::trace!("node_stats docs for {}: {}", node_id, docs.len());
        }

        if let Err(err) = exporter.write(&mut summary, &mut docs).await {
            log::error!("Failed to write node_stats: {}", err);
        }
        log::debug!("node_stats docs: {}", summary.docs);
        summary
    }
}
