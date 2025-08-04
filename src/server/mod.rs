mod api;
mod api_key;
mod file_upload;
mod index;
mod service_link;
mod template;

use super::processor::Identifiers;
use crate::{
    data::Uri,
    exporter::Exporter,
    processor::{Job, JobFailed, JobProcessing},
};
use askama::Template;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart},
    http::{HeaderMap, StatusCode},
    response::sse::Event,
    routing::{get, post},
};
use bytes::Bytes;
use datastar::prelude::{ElementPatchMode, PatchElements, PatchSignals};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    convert::Infallible,
    net::SocketAddr,
    sync::Arc,
};
use tokio::sync::{RwLock, mpsc, oneshot};

static DATASTAR_JS: &str = include_str!("web/datastar.js");
static DATASTAR_JS_MAP: &str = include_str!("web/datastar.js.map");
static ESDIAG_SVG: &str = include_str!("web/esdiag.svg");
static SCRIPT_JS: &str = include_str!("web/script.js");
static STYLE_CSS: &str = include_str!("web/style.css");

#[derive(Debug, Deserialize, Serialize)]
struct UploadServiceRequest {
    metadata: Identifiers,
    token: String,
    url: String,
}

impl From<UploadServiceRequest> for Identifiers {
    fn from(request: UploadServiceRequest) -> Self {
        Identifiers {
            account: request.metadata.account.clone(),
            case_number: request.metadata.case_number,
            filename: request.metadata.filename.clone(),
            opportunity: None,
            user: request.metadata.user,
        }
    }
}

#[derive(Clone)]
pub struct Server {
    server_handle: Option<Arc<tokio::task::JoinHandle<()>>>,
    worker_handle: Option<Arc<tokio::task::JoinHandle<()>>>,
    shutdown_signal: Option<Arc<oneshot::Sender<()>>>,
    pub rx: Option<Arc<RwLock<mpsc::Receiver<(Identifiers, Bytes)>>>>,
    pub state: Arc<ServerState>,
}

impl Server {
    pub fn new(port: u16, exporter: Exporter, kibana: String) -> Self {
        let (tx, rx) = mpsc::channel::<(Identifiers, Bytes)>(1);
        let rx = Arc::new(RwLock::new(rx));
        let rx_clone = rx.clone();

        // Create shared state
        let state = Arc::new(ServerState {
            upload_tx: tx.clone(),
            job: JobState {
                current: Arc::new(RwLock::new(None)),
                history: Arc::new(RwLock::new(Vec::with_capacity(100))),
                queue: Arc::new(RwLock::new(VecDeque::with_capacity(10))),
            },
            signals: Arc::new(RwLock::new(Signals::default())),
            exporter: Arc::new(RwLock::new(exporter)),
            kibana,
            stats: Arc::new(RwLock::new(Stats::default())),
            uploads: Arc::new(RwLock::new(HashMap::new())),
        });

        // State pointers to move into closures
        let state_index = state.clone();
        let state_upload_submit = state.clone();
        let state_upload_process = state.clone();
        let state_upload_service = state.clone();
        let state_api_key = state.clone();
        let state_api_upload = state.clone();
        let state_api_upload_service = state.clone();

        // Start the Axum server
        let handle = tokio::spawn(async move {
            // Handlers
            let upload_submit_handler = {
                move |headers, multipart: Multipart| async move {
                    file_upload::submit_handler(headers, multipart, state_upload_submit).await
                }
            };
            let upload_process_handler = {
                move |headers, signals| async move {
                    file_upload::process_hanlder(headers, signals, state_upload_process).await
                }
            };
            let service_link_handler = {
                move |headers, multipart: Multipart| async move {
                    service_link::handler(headers, multipart, state_upload_service).await
                }
            };
            let api_key_handler = {
                move |headers, signals| async move {
                    api_key::handler(headers, signals, state_api_key).await
                }
            };

            // API Hanlders
            let api_upload_handler = {
                move |headers, multipart: Multipart| async move {
                    api::upload_handler(headers, multipart, state_api_upload).await
                }
            };
            let api_service_link_handler = {
                move |headers, json: Json<UploadServiceRequest>| async move {
                    api::service_link_handler(headers, json, state_api_upload_service).await
                }
            };
            let datastar_handler = async move || {
                (
                    StatusCode::OK,
                    [("Content-Type", "text/javascript")],
                    DATASTAR_JS,
                )
            };

            let datastar_map_handler = async move || {
                (
                    StatusCode::OK,
                    [("Content-Type", "application/json")],
                    DATASTAR_JS_MAP,
                )
            };

            let index_handler =
                { move |headers| async move { index::handler(headers, state_index).await } };

            let logo_handler = async move || {
                (
                    StatusCode::OK,
                    [("Content-Type", "image/svg+xml")],
                    ESDIAG_SVG,
                )
            };

            let script_handler = async move || {
                (
                    StatusCode::OK,
                    [("Content-Type", "application/javascript")],
                    SCRIPT_JS,
                )
            };

            let style_handler =
                async move || (StatusCode::OK, [("Content-Type", "text/css")], STYLE_CSS);

            const ONE_GIBIBYTE: usize = 1024 * 1024 * 1024;
            let app = Router::new()
                .route("/", get(index_handler))
                .route("/style.css", get(style_handler))
                .route("/script.js", get(script_handler))
                .route("/datastar.js", get(datastar_handler))
                .route("/datastar.js.map", get(datastar_map_handler))
                .route("/favicon.ico", get(logo_handler))
                .route("/esdiag.svg", get(logo_handler))
                .route("/upload/submit", post(upload_submit_handler))
                .route("/upload/process", post(upload_process_handler))
                .route("/service_link", post(service_link_handler))
                .route("/api_key", post(api_key_handler))
                .route("/api/service_link", post(api_service_link_handler))
                .route("/api/upload", post(api_upload_handler))
                .layer(DefaultBodyLimit::max(ONE_GIBIBYTE));

            let addr = SocketAddr::from(([0, 0, 0, 0], port));

            // Start the server
            log::info!("Listening on port {}", port);
            match axum_server::bind(addr).serve(app.into_make_service()).await {
                Ok(_) => log::info!("Server shutdown"),
                Err(e) => log::error!("Server error: {}", e),
            }
        });

        let mut server = Self {
            server_handle: Some(Arc::new(handle)),
            worker_handle: None,
            shutdown_signal: None,
            rx: Some(rx_clone),
            state,
        };

        server.start_worker();
        server
    }

    pub async fn shutdown(&mut self) {
        // Send shutdown signal to worker thread if it exists
        if let Some(tx) = self.shutdown_signal.take() {
            log::debug!("Sending shutdown signal to worker thread");
            if let Err(e) = Arc::try_unwrap(tx).map(|tx| tx.send(())) {
                log::warn!("Failed to send shutdown signal to worker thread: {:?}", e);
            }
        }

        // Wait for worker thread to complete if it exists
        if let Some(handle) = self.worker_handle.take() {
            log::debug!("Waiting for worker thread to complete");

            // Use a timeout to avoid waiting forever
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                Arc::try_unwrap(handle).unwrap(),
            )
            .await
            {
                Ok(result) => match result {
                    Ok(_) => log::info!("Worker thread shut down successfully"),
                    Err(e) => log::warn!("Error joining worker thread: {:?}", e),
                },
                Err(_) => log::warn!("Timeout waiting for worker thread to shut down"),
            }
        }

        // Shutdown the main server
        if let Some(handle) = self.server_handle.take() {
            Arc::try_unwrap(handle).map(|handle| handle.abort()).ok();
            log::debug!("Server thread aborted");
        }
    }

    // Start a thread to process diagnostics in the background
    fn start_worker(&mut self) {
        let state = self.state.clone();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        self.shutdown_signal = Some(Arc::new(shutdown_tx));

        let handle = tokio::spawn(async move {
            log::info!("Starting diagnostic worker thread");

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        log::debug!("Worker thread received shutdown signal");
                        break;
                    }
                    _ = async {
                        if state.job.process().await == false {
                            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                        }
                    } => {}
                }
            }
        });

        self.worker_handle = Some(Arc::new(handle));
        log::debug!("Diagnostic worker thread started");
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new(3000, Exporter::default(), String::new())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Abort the server thread if it exists
        if let Some(handle) = self.server_handle.take() {
            Arc::try_unwrap(handle).map(|handle| handle.abort()).ok();
        }

        // Send shutdown signal to worker thread if it exists
        if let Some(tx) = self.shutdown_signal.take() {
            if let Err(e) = Arc::try_unwrap(tx).map(|tx| tx.send(())) {
                log::warn!("Failed to send shutdown signal to worker thread: {:?}", e);
            }
        }

        log::info!("Server dropped, server and worker threads are being shut down");
    }
}

pub struct ServerState {
    pub exporter: Arc<RwLock<Exporter>>,
    pub kibana: String,
    pub job: JobState,
    pub signals: Arc<RwLock<Signals>>,
    pub uploads: Arc<RwLock<HashMap<u64, (String, Bytes)>>>,
    stats: Arc<RwLock<Stats>>,
    upload_tx: mpsc::Sender<(Identifiers, Bytes)>,
}

impl ServerState {
    pub async fn record_success(&self, docs: u32, errors: u32) {
        let mut stats = self.stats.write().await;
        stats.docs.total += docs as u64;
        stats.docs.errors += errors as u64;
        stats.jobs.total += 1;
        stats.jobs.success += 1;
    }

    pub async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        stats.jobs.total += 1;
        stats.jobs.failed += 1;
    }

    pub async fn get_stats(&self) -> Stats {
        self.stats.read().await.clone()
    }

    pub async fn push_upload(
        &self,
        id: u64,
        filename: String,
        data: Bytes,
    ) -> Option<(String, Bytes)> {
        self.uploads.write().await.insert(id, (filename, data))
    }

    pub async fn pop_upload(&self, id: u64) -> Option<(String, Bytes)> {
        self.uploads.write().await.remove(&id)
    }
}

impl ServerState {
    pub async fn is_processing(&self) -> bool {
        self.job.current.read().await.is_some()
    }

    pub async fn queue_size(&self) -> usize {
        self.job.queue.read().await.len()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Stats {
    pub docs: DocStats,
    pub jobs: JobStats,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            docs: DocStats {
                total: 0,
                errors: 0,
            },
            jobs: JobStats {
                total: 0,
                success: 0,
                failed: 0,
            },
        }
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = match serde_json::to_string(self) {
            Ok(json) => json,
            Err(_) => return Err(std::fmt::Error),
        };
        write!(f, "{}", json)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocStats {
    pub total: u64,
    pub errors: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JobStats {
    pub total: u64,
    pub success: u64,
    pub failed: u64,
}

pub struct JobState {
    current: Arc<RwLock<Option<JobProcessing>>>,
    history: Arc<RwLock<Vec<Job>>>,
    queue: Arc<RwLock<VecDeque<JobProcessing>>>,
}

impl JobState {
    pub async fn process(&self) -> bool {
        let job = match self.queue.write().await.pop_front() {
            Some(job) => {
                log::info!("Processing job {} from queue", job.id);
                job
            }
            None => {
                log::trace!("No jobs in queue");
                return false;
            }
        };

        {
            self.current.write().await.replace(job.clone());
        }

        match job.process().await {
            Ok(job_completed) => {
                log::info!(
                    "Job {} completed in {:.3} seconds",
                    job_completed.id,
                    job_completed.processing_seconds()
                );
                let mut history = self.history.write().await;
                history.push(Job::Completed(job_completed));
                let mut current = self.current.write().await;
                *current = None;
                true
            }
            Err(job_failed) => {
                log::error!(
                    "Job {} failed with error: {}",
                    job_failed.id,
                    job_failed.error
                );
                let mut history = self.history.write().await;
                history.push(Job::Failed(job_failed));
                let mut current = self.current.write().await;
                *current = None;
                false
            }
        }
    }

    pub async fn push(&self, job: JobProcessing) {
        let mut queue = self.queue.write().await;
        log::debug!("Adding job {} to queue {}", job.id, queue.len());
        queue.push_back(job);
    }

    pub async fn record_failure(&self, job: JobFailed) {
        log::error!("Job {} failed with error: {}", job.id, job.error);
        self.history.write().await.push(Job::Failed(job));
    }
}

#[derive(Debug, Deserialize)]
pub struct Signals {
    pub processing: bool,
    pub uploading: bool,
    pub es_api: EsApiKey,
    pub service_link: ServiceLink,
    pub stats: Stats,
    pub file_upload: FileUpload,
}

impl Default for Signals {
    fn default() -> Self {
        Signals {
            processing: false,
            uploading: false,
            es_api: EsApiKey {
                key: String::new(),
                url: Uri::default(),
            },
            service_link: ServiceLink {
                url: Uri::default(),
                token: String::new(),
                filename: String::new(),
            },
            stats: Stats::default(),
            file_upload: FileUpload { job_id: 0 },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EsApiKey {
    pub key: String,
    pub url: Uri,
}

#[derive(Debug, Deserialize)]
pub struct FileUpload {
    pub job_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct ServiceLink {
    pub url: Uri,
    pub token: String,
    pub filename: String,
}

pub fn patch_signals(signals: &str) -> Result<Event, Infallible> {
    let sse_event = PatchSignals::new(signals).write_as_axum_sse_event();
    Ok(sse_event)
}

pub fn patch_template(template: impl Template) -> Result<Event, Infallible> {
    let element = template.render().expect("Failed to render template");
    let sse_event = PatchElements::new(element).write_as_axum_sse_event();
    Ok(sse_event)
}

pub fn patch_job_feed(template: impl Template) -> Result<Event, Infallible> {
    let element = template.render().expect("Failed to render template");
    let sse_event = PatchElements::new(element)
        .selector("#job-feed")
        .mode(ElementPatchMode::After)
        .write_as_axum_sse_event();
    Ok(sse_event)
}

fn get_user_email(headers: &HeaderMap) -> Option<String> {
    match std::env::var("ESDIAG_USER").ok() {
        Some(user) => Some(user),
        None => headers
            .get("X-Goog-Authenticated-User-Email")
            .and_then(|value| value.to_str().ok())
            .map(|email| {
                // Google auth headers are typically in format "accounts.google.com:email"
                email.split(':').last().unwrap_or(email).to_string()
            }),
    }
}
