// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

/// Collect diagnostic data from applications
mod collector;
/// Universal diagnostic processor
mod diagnostic;
/// Processors for Elastic Cloud Kubernetes (ECK) diagnostics
//mod elastic_cloud_kubernetes;
/// Processors for Elasticsearch diagnostics
mod elasticsearch;
/// Processors for Managed Kubernetes Infrastructure (MKI) platform diagnostics
//mod kubernetes_platform;
/// Processors for Logstash diagnostics
mod logstash;
pub use collector::Collector;
pub use diagnostic::{
    DataSource, DiagnosticManifest, DiagnosticReport, Manifest, Product,
    data_source::PathType,
    manifest::ManifestBuilder,
    report::{BatchResponse, Identifiers, ProcessorSummary},
};
pub use elasticsearch::Cluster as ElasticsearchCluster;

use crate::{exporter::Exporter, receiver::Receiver};
//use elastic_cloud_kubernetes::ElasticCloudKubernetesDiagnostic;
use elasticsearch::ElasticsearchDiagnostic;
use eyre::{Result, eyre};
//use kubernetes_platform::KubernetesPlatformDiagnostic;
use logstash::LogstashDiagnostic;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::sync::mpsc;

pub struct Processor<S: State> {
    receiver: Arc<Receiver>,
    exporter: Arc<Exporter>,
    start_time: std::time::Instant,
    id: u64,
    pub state: S,
}

/// The `Ready` state represents a new processing job
pub struct Ready {
    manifest: DiagnosticManifest,
    identifiers: Identifiers,
}

impl Ready {
    fn fail(self, runtime: u128, error: String) -> Failed {
        Failed { error, runtime }
    }
}

/// The `Processing` state represents an active processing job
pub struct Processing {
    diagnostic: Diagnostic,
    progress: Progress,
    tx: mpsc::Sender<Progress>,
}

impl Progress {
    fn new() -> Self {
        Progress {
            percent: 0.0,
            docs: 0,
            errors: 0,
            processors: 0,
        }
    }

    fn update(&mut self, percent: f32, stats: &Stats) {
        self.percent = percent;
        self.docs = stats.docs;
        self.errors = stats.errors;
        self.processors = stats.processors;
    }
}

/// The counters we track for progress reporting
// We implment copy to simplify progress channel sending
#[derive(Copy, Clone)]
pub struct Progress {
    pub percent: f32,
    pub docs: usize,
    pub errors: usize,
    pub processors: usize,
}

/// The `Completed` state represents a succesfull processing job
pub struct Completed {
    report: DiagnosticReport,
    pub runtime: u128,
}

/// The final totals for the completed processing job
struct Stats {
    docs: usize,
    errors: usize,
    processors: usize,
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Total: {}, errors: {}, processors: {}",
            self.docs, self.errors, self.processors
        )
    }
}

/// The `Failed` state represents a failed processing job
pub struct Failed {
    error: String,
    pub runtime: u128,
}

/// The `Status` trait represents the state of a processing job
pub trait State {}
// The Status trait doesn't need any functions, it is only used for trait bounds
impl State for Ready {}
impl State for Processing {}
impl State for Completed {}
impl State for Failed {}

impl Processor<Ready> {
    /// Try creating a processor with the receiver, exporter and identifiers.
    /// Will attempt to build a manifest from a call to the receiver.
    pub async fn try_new(
        receiver: Receiver,
        exporter: Exporter,
        identifiers: Identifiers,
    ) -> Result<Self> {
        let manifest = receiver.try_get_manifest().await?;
        Ok(Self {
            receiver: Arc::new(receiver),
            exporter: Arc::new(exporter),
            id: new_job_id(),
            start_time: std::time::Instant::now(),
            state: Ready {
                manifest,
                identifiers,
            },
        })
    }

    /// State transition from `Ready` to `Processing`, returning the progress channel
    pub async fn start(
        self,
    ) -> Result<(Processor<Processing>, mpsc::Receiver<Progress>), Processor<Failed>> {
        log::debug!("Transitioned: Processor<Processing>");
        let (tx, rx) = mpsc::channel::<Progress>(10);
        match Diagnostic::try_new(
            self.receiver.clone(),
            self.exporter.clone(),
            self.state.manifest,
        )
        .await
        {
            Ok(diagnostic) => {
                let processor = Processor {
                    receiver: self.receiver,
                    exporter: self.exporter,
                    id: self.id,
                    start_time: self.start_time,
                    state: Processing {
                        diagnostic,
                        tx,
                        progress: Progress::new(),
                    },
                };
                Ok((processor, rx))
            }
            Err(err) => Err(Processor {
                receiver: self.receiver,
                exporter: self.exporter,
                start_time: self.start_time,
                id: self.id,
                state: Failed {
                    runtime: self.start_time.elapsed().as_millis(),
                    error: err.to_string(),
                },
            }),
        }
    }
}

/// The actively `Processing` state.
impl Processor<Processing> {
    pub async fn process(self) -> Result<Processor<Completed>, Processor<Failed>> {
        log::debug!("Processing with async progress updates");

        let report = match self.state.diagnostic {
            Diagnostic::Elasticsearch(diagnostic) => diagnostic.run().await,
            //Diagnostic::ElasticCloudKubernetes(diagnostic) => diagnostic.run().await,
            //Diagnostic::KubernetesPlatform(diagnostic) => diagnostic.run().await,
            //Diagnostic::Kibana(diagnostic) => diagnostic.run().await?,
            Diagnostic::Logstash(diagnostic) => diagnostic.run().await,
        };

        let mut report = match report {
            Ok(report) => report,
            Err(err) => {
                return Err(Processor {
                    receiver: self.receiver,
                    exporter: self.exporter,
                    start_time: self.start_time,
                    id: self.id,
                    state: Failed {
                        runtime: self.start_time.elapsed().as_millis(),
                        error: err.to_string(),
                    },
                });
            }
        };

        log::info!(
            "Created {} documents for {} diagnostic: {}",
            report.docs.created,
            report.product,
            report.metadata.id,
        );
        if let Ok(kibana_url) = std::env::var("ESDIAG_KIBANA_URL") {
            let kibana_link = format!(
                "{}/app/dashboards#/view/4e0a26b2-e5f8-4b58-b617-86f5cdd0edad?_g=(filters:!(('$state':(store:globalState),meta:(disabled:!f,index:'4319ebc4-df81-4b18-b8bd-6aaa55a1fd13',key:diagnostic.id,negate:!f,params:(query:'{}'),type:phrase),query:(match_phrase:(diagnostic.id:'{}')))),refreshInterval:(pause:!t,value:60000),time:(from:now-90d,to:now))",
                kibana_url, report.metadata.id, report.metadata.id
            );
            log::info!("{}", kibana_link);
            report.add_kibana_link(kibana_link);
        }
        let runtime = self.start_time.elapsed().as_millis();
        report.add_processing_duration(runtime);

        Ok(Processor {
            exporter: self.exporter,
            receiver: self.receiver,
            start_time: self.start_time,
            id: self.id,
            state: Completed {
                report,
                runtime: self.start_time.elapsed().as_millis(),
            },
        })
    }
}

impl std::fmt::Display for Processor<Failed> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Processor {} failed: {}", self.id, self.state.error)
    }
}

impl Processor<Completed> {
    pub fn report(&self) -> &DiagnosticReport {
        &self.state.report
    }
}

// -------- Legacy implementation ---------
pub enum Diagnostic {
    Elasticsearch(Box<ElasticsearchDiagnostic>),
    // ElasticCloudKubernetes(Box<ElasticCloudKubernetesDiagnostic>),
    // KubernetesPlatform(Box<KubernetesPlatformDiagnostic>),
    //Kibana(KibanaDiagnostic)
    Logstash(Box<LogstashDiagnostic>),
}

impl Diagnostic {
    pub async fn try_new(
        receiver: Arc<Receiver>,
        exporter: Arc<Exporter>,
        manifest: DiagnosticManifest,
    ) -> Result<Self> {
        log::info!("Processing {} diagnostic", manifest.product);
        log::trace!(
            "Diagnostic Manifest: {}",
            serde_json::to_string(&manifest).unwrap()
        );
        match manifest.product {
            Product::Elasticsearch => {
                let diagnostic = ElasticsearchDiagnostic::new(receiver, exporter, manifest).await?;
                Ok(Self::Elasticsearch(diagnostic))
            }
            //Product::ECK => {
            //    let diagnostic = ElasticCloudKubernetesDiagnostic::new(&receiver, manifest).await?;
            //    Ok(Self::ElasticCloudKubernetes(diagnostic))
            //}
            //Product::KubernetesPlatform => {
            //    let diagnostic = KubernetesPlatformDiagnostic::new(&receiver, manifest).await?;
            //    Ok(Self::KubernetesPlatform(diagnostic))
            //}
            Product::Logstash => {
                let diagnostic = LogstashDiagnostic::new(receiver, exporter, manifest).await?;
                Ok(Self::Logstash(diagnostic))
            }
            _ => Err(eyre!("Unsupported product or diagnostic bundle")),
        }
    }

    pub async fn run(self) -> Result<DiagnosticReport> {
        let start_time = std::time::Instant::now();
        let mut report = match self {
            Self::Elasticsearch(diagnostic) => diagnostic.run().await?,
            //Self::ElasticCloudKubernetes(diagnostic) => diagnostic.run().await?,
            //Self::KubernetesPlatform(diagnostic) => diagnostic.run().await?,
            //Self::Kibana(diagnostic) => diagnostic.run().await?,
            Self::Logstash(diagnostic) => diagnostic.run().await?,
        };
        log::info!(
            "Created {} documents for {} diagnostic: {}",
            report.docs.created,
            report.product,
            report.metadata.id,
        );
        if let Ok(kibana_url) = std::env::var("ESDIAG_KIBANA_URL") {
            let kibana_link = format!(
                "{}/app/dashboards#/view/4e0a26b2-e5f8-4b58-b617-86f5cdd0edad?_g=(filters:!(('$state':(store:globalState),meta:(disabled:!f,index:'4319ebc4-df81-4b18-b8bd-6aaa55a1fd13',key:diagnostic.id,negate:!f,params:(query:'{}'),type:phrase),query:(match_phrase:(diagnostic.id:'{}')))),refreshInterval:(pause:!t,value:60000),time:(from:now-90d,to:now))",
                kibana_url, report.metadata.id, report.metadata.id
            );
            log::info!("{}", kibana_link);
            report.add_kibana_link(kibana_link);
        }
        report.add_processing_duration(start_time.elapsed().as_millis());
        Ok(report)
    }
}

trait DocumentExporter<T, U> {
    async fn documents_export(
        self,
        exporter: &Exporter,
        lookups: &T,
        metadata: &U,
    ) -> ProcessorSummary;
}

trait DiagnosticProcessor {
    async fn new(
        receiver: Arc<Receiver>,
        exporter: Arc<Exporter>,
        manifest: DiagnosticManifest,
    ) -> Result<Box<Self>>;
    async fn run(self) -> Result<DiagnosticReport>;
    #[allow(dead_code)]
    fn id(&self) -> &str;
}

trait Metadata {
    fn as_meta_doc(&self) -> serde_json::Value;
}

pub fn new_job_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        % 100000
}
