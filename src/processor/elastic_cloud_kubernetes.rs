use super::{lookup::Lookup, DiagnosticProcessor, ElasticsearchDiagnostic};
use crate::{
    data::diagnostic::{DiagPath, DiagnosticManifest},
    exporter::Exporter,
    receiver::Receiver,
};
use color_eyre::eyre::Result;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct ElasticCloudKubernetesDiagnostic {
    lookups: Arc<Lookups>,
    #[serde(skip)]
    exporter: Arc<Exporter>,
    #[serde(skip)]
    receiver: Arc<Receiver>,
    included_diagnostics: Vec<DiagPath>,
}

impl DiagnosticProcessor for ElasticCloudKubernetesDiagnostic {
    async fn new(
        mut manifest: DiagnosticManifest,
        receiver: Receiver,
        exporter: Exporter,
    ) -> Result<Box<Self>> {
        let lookups = Arc::new(Lookups {
            k8s_node: Lookup::new(),
        });

        log::debug!(
            "Eck diagnostic includes: {:?}",
            &manifest.included_diagnostics
        );

        let included_diagnostics = match manifest.included_diagnostics.take() {
            Some(diags) => diags,
            None => vec![],
        };

        Ok(Box::new(Self {
            lookups,
            exporter: Arc::new(exporter),
            receiver: Arc::new(receiver),
            included_diagnostics,
        }))
    }

    async fn run(self) -> Result<(String, usize)> {
        self.receiver.is_connected().await;
        for diagnostic in self.included_diagnostics {
            match diagnostic.diag_type.as_str() {
                "elasticsearch" => {
                    log::info!(
                        "Processing {} diagnostic at {}",
                        diagnostic.diag_type,
                        diagnostic.diag_path
                    );
                    let receiver = self.receiver.clone_for_subdir(&diagnostic.diag_path)?;
                    let manifest = receiver.try_get_manifest().await?;
                    let diagnostic =
                        ElasticsearchDiagnostic::new(manifest, receiver, self.exporter.cloned())
                            .await?;
                    let (diag_id, doc_count) = diagnostic.run().await?;
                    log::info!(
                        "Created {} documents for diagnostic: {}",
                        doc_count,
                        diag_id
                    );
                }
                _ => {
                    log::warn!(
                        "Skipping {} diagnostic at {}",
                        diagnostic.diag_type,
                        diagnostic.diag_path
                    );
                }
            }
        }

        Ok(("eck-diagnostic".to_string(), 0))
    }

    async fn process_queue(&self) -> usize {
        0
    }
}

#[derive(Serialize)]
pub struct Lookups {
    pub k8s_node: Lookup<String>,
}
