// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use crate::{exporter::Exporter, processor::ProcessorSummary};

use super::{
    super::{DocumentExporter, ElasticsearchMetadata, Lookups, Metadata},
    IndexSettings, IndicesSettings,
};
use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;

impl DocumentExporter<Lookups, ElasticsearchMetadata> for IndicesSettings {
    async fn documents_export(
        mut self,
        exporter: &Exporter,
        lookups: &Lookups,
        metadata: &ElasticsearchMetadata,
    ) -> ProcessorSummary {
        log::debug!("processing indices: {}", self.len());
        let index_metadata = metadata.for_data_stream("settings-index-esdiag");
        let collection_date = metadata.timestamp;

        let index_settings: Vec<Value> = self
            .par_drain()
            .filter_map(|(name, settings)| {
                let index_settings = settings
                    .settings
                    .index
                    .data_stream(lookups.data_stream.by_id(&name).cloned())
                    .age(collection_date)
                    .name(name)
                    .build();

                serde_json::to_value(EnrichedIndexSettings {
                    index: index_settings,
                    metadata: index_metadata.as_meta_doc(),
                })
                .ok()
            })
            .collect();

        log::debug!("index setting docs: {}", index_settings.len());
        let mut summary = ProcessorSummary::new(index_metadata.data_stream.to_string());
        if let Err(err) = exporter.write(&mut summary, index_settings).await {
            log::error!("Failed to write index settings: {}", err);
        }
        summary
    }
}

#[derive(Clone, Serialize)]
struct EnrichedIndexSettings {
    index: IndexSettings,
    #[serde(flatten)]
    metadata: Value,
}
