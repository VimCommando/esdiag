use super::{ServerState, get_user_email, patch_signals, template};
use crate::{
    data::{Uri, diagnostic::report::Identifiers},
    processor::JobNew,
    receiver::Receiver,
};
use askama::Template;
use async_stream::stream;
use axum::{
    extract::Multipart,
    http::HeaderMap,
    response::{IntoResponse, Sse},
};
use datastar::{consts::ElementPatchMode, prelude::PatchElements};
use std::{convert::Infallible, sync::Arc};
use url::Url;

pub async fn handler(
    headers: HeaderMap,
    mut multipart: Multipart,
    state: Arc<ServerState>,
) -> impl IntoResponse {
    // Extract authenticated user email from header
    let username = get_user_email(&headers);

    let mut token = String::new();
    let mut url = String::new();
    let mut filename = String::new();

    // Process the multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("");
        match field_name {
            "token" => {
                token = field.text().await.unwrap_or_default();
            }
            "url" => {
                url = field.text().await.unwrap_or_default();
            }
            "filename" => {
                filename = field.text().await.unwrap_or_default();
            }
            _ => {} // Ignore other fields
        }
    }

    log::info!("Received Elastic upload service request for: {}", url);

    Sse::new(stream! {
        yield patch_signals(r#"{"uploading":true}"#);

        // Construct the URL with token authentication
        let uploader_service_url = match Url::parse(&url) {
            Ok(mut url) => {
                // Set username to "token" and password to the actual token
                if url.set_username("token").is_err() {
                    let element = template::Error::new(
                        "error-url",
                        "Upload Service",
                        "Failed to set username in URL",
                    );
                    let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                    yield Ok::<_, Infallible>(sse_event);
                    yield patch_signals(r#"{"uploading":false}"#);
                }
                if url.set_password(Some(&token)).is_err() {
                    let element = template::Error::new(
                        "error-url",
                        "Upload Service",
                        "Failed to set token in URL",
                    );
                    let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                    yield Ok::<_, Infallible>(sse_event);
                    yield patch_signals(r#"{"uploading":false}"#);
                }
                url
            }
            Err(e) => {
                let error_msg = format!("Invalid URL: {}", e);
                log::error!("Invalid URL provided: {}", e);
                let element = template::Error::new(
                    "error-url",
                    "Upload Service",
                    &error_msg
                );
                let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                yield Ok::<_, Infallible>(sse_event);
                yield patch_signals(r#"{"uploading":false}"#);
                return
            }
        };

        // Create URI from the URL
        let uri = match Uri::try_from(uploader_service_url.to_string()) {
            Ok(uri) => uri,
            Err(e) => {
                let error_msg = format!("Failed to create URI: {}", e);
                log::error!("Failed to create URI: {}", e);
                let element = template::Error::new(
                    "error-url",
                    "Upload Service",
                    &error_msg
                );
                let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                yield Ok::<_, Infallible>(sse_event);
                yield patch_signals(r#"{"uploading":false}"#);
                return
            }
        };

        // Create receiver from URI
        let receiver = match Receiver::try_from(uri) {
            Ok(receiver) => receiver,
            Err(e) => {
                state.record_failure().await;
                let error_msg = format!("Failed to create receiver: {}", e);
                log::error!("Failed to create receiver: {}", e);
                let element = template::JobFailed::element(None, &error_msg, &filename);
                let sse_event = PatchElements::new(element).selector("#job-feed")
                    .mode(ElementPatchMode::After).write_as_axum_sse_event();
                yield Ok::<_, Infallible>(sse_event);
                yield patch_signals(r#"{"uploading":false}"#);
                return
            }
        };

        let identifiers = Identifiers {
            account: None,
            case_number: None,
            filename: Some(filename.clone()),
            opportunity: None,
            user: username,
        };

        let exporter = {
            state.exporter.read().await.clone().with_identifiers(identifiers)
        };

        match JobNew::new(&exporter.identifiers(), receiver).ready(exporter).await {
            Ok(job) => {
                let job = job.start();
                let element = template::JobProcessing {
                    job_id: job.id,
                    filename: &job.filename,
                }.render().expect("Failed to render JobProcessing template");
                let sse_event = PatchElements::new(element).selector("#job-feed")
                    .mode(ElementPatchMode::After).write_as_axum_sse_event();
                yield patch_signals(r#"{"uploading":false,"processing":true}"#);
                yield Ok::<_, Infallible>(sse_event);

                let elements = match job.process().await {
                    Ok(job) => {
                        state.record_success(job.report.docs.total, job.report.docs.errors).await;
                        template::JobCompleted {
                            job_id: job.id,
                            diagnostic_id: &job.report.metadata.id,
                            docs_created: &job.report.docs.created,
                            filename: &job.filename,
                            kibana_link: job.report.kibana_link.as_ref().unwrap_or(&"#".to_string()),
                            product: &job.report.product.to_string(),
                        }.render().unwrap_or(template::Error::new("error", "Render error", "Failed to render template"))
                    },
                    Err(job) => {
                        state.record_failure().await;
                        let elements = template::JobFailed {
                            job_id: job.id,
                            error: &job.error,
                            filename: &job.filename,
                        }.render().expect("Failed to render JobFailed template");
                        state.job.record_failure(job).await;
                        elements
                    }
                };
                let sse_event = PatchElements::new(elements).write_as_axum_sse_event();
                yield Ok::<_, Infallible>(sse_event);
                yield patch_signals(r#"{"processing":false}"#);
            },
            Err(job) => {
                state.record_failure().await;
                let element = template::JobFailed {
                    job_id: job.id,
                    error: &job.error,
                    filename: &job.filename,
                }.render().expect("Failed to render JobFailed template");
                state.job.record_failure(job).await;
                let sse_event = PatchElements::new(element).selector("#job-feed")
                    .mode(ElementPatchMode::After).write_as_axum_sse_event();
                yield Ok::<_, Infallible>(sse_event);
                yield patch_signals(r#"{"processing":false}"#);
            },
        };

        let signals = format!(r#"{{"processing":false,"stats":{}}}"#, state.get_stats().await);
        yield patch_signals(&signals);
    })
}
