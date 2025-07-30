use super::ServerState;
use crate::{
    data::{Uri, diagnostic::report::Identifiers},
    exporter::Exporter,
    processor::JobNew,
    receiver::Receiver,
};
use axum::{
    extract::Multipart,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
};
use std::sync::Arc;
use url::Url;

pub async fn handler(
    headers: HeaderMap,
    mut multipart: Multipart,
    state: Arc<ServerState>,
) -> impl IntoResponse {
    let mut token = String::new();
    let mut url = String::new();
    let mut filename: Option<String> = None;

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
                filename = Some(field.text().await.unwrap_or_default());
            }
            _ => {} // Ignore other fields
        }
    }

    log::info!("Received elastic uploader request for: {}", url);

    // Extract authenticated user email from header
    let username = headers
        .get("X-Goog-Authenticated-User-Email")
        .and_then(|value| value.to_str().ok())
        .map(|email| {
            // Google auth headers are typically in format "accounts.google.com:email"
            email.split(':').last().unwrap_or(email).to_string()
        });

    // Construct the URL with token authentication
    let uploader_service_url = match Url::parse(&url) {
        Ok(mut url) => {
            // Set username to "token" and password to the actual token
            if url.set_username("token").is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(format!(
                        r#"<div class="status-box error">
                            🛑 <b>Error:</b> Failed to set token in URL
                        </div>"#
                    )),
                )
                    .into_response();
            }
            if url.set_password(Some(&token)).is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Html(format!(
                        r#"<div class="status-box error">
                            🛑 <b>Error:</b> Failed to set token in URL
                        </div>"#
                    )),
                )
                    .into_response();
            }
            url
        }
        Err(e) => {
            let error_msg = format!("Invalid URL: {}", e);
            log::error!("Invalid URL provided: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<div class="status-box error">
                        🛑 <b>Error:</b> {}
                    </div>"#,
                    error_msg
                )),
            )
                .into_response();
        }
    };

    // Create URI from the URL
    let uri = match Uri::try_from(uploader_service_url.to_string()) {
        Ok(uri) => uri,
        Err(e) => {
            let error_msg = format!("Failed to create URI: {}", e);
            log::error!("Failed to create URI: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<div class="status-box error">
                        🛑 <b>Error:</b> {}
                    </div>"#,
                    error_msg
                )),
            )
                .into_response();
        }
    };

    // Create receiver from URI
    let receiver = match Receiver::try_from(uri) {
        Ok(receiver) => receiver,
        Err(e) => {
            let error_msg = format!("Failed to create receiver: {}", e);
            log::error!("Failed to create receiver: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    r#"<div class="status-box error">
                        🛑 <b>Error:</b> {}
                    </div>"#,
                    error_msg
                )),
            )
                .into_response();
        }
    };

    let identifiers = Identifiers {
        account: None,
        case_number: None,
        filename: filename.clone(),
        opportunity: None,
        user: username,
    };

    let exporter = match Exporter::try_from(None) {
        Ok(exporter) => exporter.with_identifiers(identifiers),
        Err(e) => {
            let error_msg = format!("Failed to create exporter: {}", e);
            log::error!("Failed to create exporter: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    r#"<div class="status-box error">
                        🛑 <b>Error:</b> {}
                    </div>"#,
                    error_msg
                )),
            )
                .into_response();
        }
    };

    let job = JobNew::new(&exporter.identifiers(), receiver);

    let job_ready = match job.ready(exporter).await {
        Ok(job_ready) => job_ready,
        Err(failed_job) => {
            let error_msg = format!("Failed to prepare job: {}", failed_job.error);
            log::error!("Failed to prepare job: {}", failed_job.error);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!(
                    r#"<div class="status-box error">
                        🛑 <b>Error:</b> {}
                    </div>"#,
                    error_msg
                )),
            )
                .into_response();
        }
    };

    // Start processing
    let job_processing = job_ready.start();

    // Add to queue
    let mut queue = state.job.queue.write().await;
    queue.push_back(job_processing);
    let queue_size = queue.len();

    log::info!("Added elastic uploader job to queue (size: {})", queue_size);

    (
        StatusCode::OK,
        Html(format!(
            r#"<div id="current-status" class="status-box processing">
                ✅ Service upload successful! Retrieving: {}
            </div>"#,
            filename.as_deref().unwrap_or("diagnostic")
        )),
    )
        .into_response()
}
