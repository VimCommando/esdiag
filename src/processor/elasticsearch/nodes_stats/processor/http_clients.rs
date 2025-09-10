// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::super::nodes::NodeDocument;
use super::{ElasticsearchMetadata, Metadata, ProcessorSummary};
use crate::exporter::Exporter;
use eyre::{OptionExt, Result};
use json_patch::merge;
use rayon::prelude::*;
use serde_json::{Value, json};

/// Extract http.clients
pub async fn extract(
    exporter: &Exporter,
    summary: &mut ProcessorSummary,
    mut clients: Value,
    metadata: &ElasticsearchMetadata,
    node_metadata: Option<&NodeDocument>,
) -> Result<()> {
    let metadata = metadata
        .for_data_stream("metrics-node.http.clients-esdiag")
        .as_meta_doc();
    let clients = clients
        .as_array_mut()
        .ok_or_eyre("Error extracting node.http.clients data")?;

    let mut docs = Vec::<Value>::with_capacity(200);
    docs.par_extend(clients.par_drain(..).map(|client| {
        let mut doc = json!({ "node": node_metadata, "http": { "client": client }});
        merge(&mut doc, &metadata);
        doc
    }));

    exporter.write(summary, &mut docs).await?;
    log::trace!("clients: {}", summary.docs);
    Ok(())
}
