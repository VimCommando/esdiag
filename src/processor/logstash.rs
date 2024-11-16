/// Logstash diagnostic metadata
mod metadata;
/// Logstash node processor
mod node;

use super::{DataProcessor, DiagnosticProcessor, Metadata};
use crate::{
    data::{
        self,
        diagnostic::DiagnosticManifest,
        logstash::{
            LogstashHotThreads, LogstashNode, LogstashNodeStats, LogstashPlugins, LogstashVersion,
        },
    },
    exporter::Exporter,
    receiver::Receiver,
};
use color_eyre::eyre::Result;
use metadata::LogstashMetadata;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct LogstashDiagnostic {
    lookups: Arc<Lookups>,
    metadata: Arc<LogstashMetadata>,
    #[serde(skip)]
    exporter: Arc<Exporter>,
    #[serde(skip)]
    receiver: Arc<Receiver>,
}

impl DiagnosticProcessor for LogstashDiagnostic {
    async fn new(
        manifest: DiagnosticManifest,
        receiver: Receiver,
        exporter: Exporter,
    ) -> Result<Box<Self>> {
        let logstash_version = receiver.get::<LogstashVersion>().await?;
        let metadata = LogstashMetadata::try_new(manifest, logstash_version)?;
        let plugins = receiver.get::<LogstashPlugins>().await?;

        Ok(Box::new(Self {
            lookups: Arc::new(Lookups {
                plugin_count: plugins.total,
            }),
            metadata: Arc::new(metadata),
            exporter: Arc::new(exporter),
            receiver: Arc::new(receiver),
        }))
    }

    async fn run(self) -> Result<(String, usize)> {
        log::debug!("Running Logstash diagnostic processors");
        if log::max_level() >= log::Level::Debug {
            data::save_file("diagnostic.json", &self)?;
        }
        let mut doc_count = 0;

        if let Ok((index, docs)) = self
            .receiver
            .get::<LogstashNode>()
            .await
            .map(|data| data.generate_docs(self.lookups.clone(), self.metadata.clone()))
        {
            match self.exporter.write(index, docs).await {
                Ok(count) => doc_count += count,
                Err(e) => log::error!("Elasticsearch exporter: {e}"),
            }
        };

        /*
                let docs = self
                    .receiver
                    .get::<LogstashNodeStats>()
                    .await
                    .map(|data| data.generate_docs(self.lookups.clone(), self.metadata.clone()));

                let docs = self.receiver
                    .get::<LogstashPlugins>()
                    .await
                    .map(|data| data.generate_docs(self.lookups.clone(), self.metadata.clone()));

                let docs = self.receiver
                    .get::<LogstashHotThreads>()
                    .await
                    .map(|data| data.generate_docs(self.lookups.clone(), self.metadata.clone()));
        */
        Ok((String::from("Logstash"), doc_count))
    }

    async fn process_queue(&self) -> usize {
        0
    }
}

#[derive(Serialize)]
struct Lookups {
    plugin_count: u32,
}
