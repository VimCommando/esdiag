// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use crate::{exporter::Exporter, processor::ProcessorSummary};

use super::{
    super::{DocumentExporter, ElasticsearchMetadata, Lookups, Metadata},
    SlmPolicies, SlmPolicy,
};
use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;

impl DocumentExporter<Lookups, ElasticsearchMetadata> for SlmPolicies {
    async fn documents_export(
        self,
        exporter: &Exporter,
        _lookups: &Lookups,
        metadata: &ElasticsearchMetadata,
    ) -> ProcessorSummary {
        log::debug!("processing SLM policies");
        let data_stream = "settings-slm-esdiag".to_string();
        let metadata = metadata.for_data_stream(&data_stream).as_meta_doc();

        let mut policies: Vec<(String, SlmPolicy)> = self.into_par_iter().collect();

        let policies: Vec<Value> = policies
            .par_drain(..)
            .filter_map(|(name, config)| {
                serde_json::to_value(SlmDoc {
                    slm: SlmPolicyDoc { name, config },
                    metadata: metadata.clone(),
                })
                .ok()
            })
            .collect();

        log::debug!("slm policy docs: {}", policies.len());
        let mut summary = ProcessorSummary::new(data_stream);
        if let Err(err) = exporter.write(&mut summary, policies).await {
            log::error!("Failed to write SLM policies: {}", err);
        }
        summary
    }
}

#[derive(Serialize)]
struct SlmDoc {
    slm: SlmPolicyDoc,
    #[serde(flatten)]
    metadata: Value,
}

#[derive(Serialize)]
struct SlmPolicyDoc {
    name: String,
    #[serde(flatten)]
    config: SlmPolicy,
}
