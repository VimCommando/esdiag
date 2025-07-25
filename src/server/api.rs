use super::{ServerState, UploadServiceRequest};
use crate::{
    data::{Uri, diagnostic::report::Identifiers},
    exporter::Exporter,
    processor::JobNew,
    receiver::Receiver,
};
use axum::{
    Json,
    extract::Multipart,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use bytes::Bytes;
use std::sync::Arc;
use url::Url;

pub async fn upload_handler(
    headers: HeaderMap,
    mut multipart: Multipart,
    state: Arc<ServerState>,
) -> impl IntoResponse {
    // Extract authenticated user email from header
    let username = headers
        .get("X-Goog-Authenticated-User-Email")
        .and_then(|value| value.to_str().ok())
        .map(|email| {
            // Google auth headers are typically in format "accounts.google.com:email"
            email.split(':').last().unwrap_or(email).to_string()
        });

    // Process the multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            // Check if the file has a valid filename
            let filename = match field.file_name() {
                Some(filename) if !filename.ends_with(".zip") => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": "Invalid file type. Only .zip files are allowed."
                        })),
                    )
                        .into_response();
                }
                Some(file_name) => file_name.to_string(),
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "No file name provided"})),
                    )
                        .into_response();
                }
            };
            // Get the file data
            match field.bytes().await {
                Ok(data) => {
                    let message = format!("Received upload: {} ({} bytes)", filename, data.len());
                    log::info!("{}", message);

                    // Clone the data to avoid ownership issues
                    let bytes = Bytes::copy_from_slice(&data);
                    let identifiers = Identifiers {
                        account: None,
                        case_number: None,
                        filename: Some(filename.clone()),
                        user: username,
                        opportunity: None,
                    };

                    // Send the bytes through the channel
                    if state.upload_tx.send((identifiers, bytes)).await.is_ok() {
                        return (
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "status": "processing",
                                "message": message,
                            })),
                        )
                            .into_response();
                    } else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({
                                "status": "error",
                                "error": "Failed to process the upload"
                            })),
                        )
                            .into_response();
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to read upload data: {}", e);
                    log::error!("{}", error_msg);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "status": "error",
                            "error": error_msg
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"status": "error", "error": "No file part in the request"})),
    )
        .into_response()
}

pub async fn upload_service_handler(
    headers: HeaderMap,
    Json(payload): Json<UploadServiceRequest>,
    state: Arc<ServerState>,
) -> impl IntoResponse {
    log::info!(
        "Received JSON elastic uploader request for: {}",
        payload.url
    );

    // Extract filename before moving payload
    let _filename = payload.metadata.filename.clone();

    // Extract authenticated user email from header
    let username = headers
        .get("X-Goog-Authenticated-User-Email")
        .and_then(|value| value.to_str().ok())
        .map(|email| {
            // Google auth headers are typically in format "accounts.google.com:email"
            email.split(':').last().unwrap_or(email).to_string()
        });

    // Construct the URL with token authentication
    let uploader_service_url = match Url::parse(&payload.url) {
        Ok(mut url) => {
            // Set username to "token" and password to the actual token
            if url.set_username("token").is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Failed to set token in URL"
                    })),
                )
                    .into_response();
            }
            if url.set_password(Some(&payload.token)).is_err() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Failed to set token in URL"
                    })),
                )
                    .into_response();
            }
            url
        }
        Err(e) => {
            log::error!("Invalid URL provided: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid URL: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Create URI from the URL
    let uri = match Uri::try_from(uploader_service_url.to_string()) {
        Ok(uri) => uri,
        Err(e) => {
            log::error!("Failed to create URI: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to create URI: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Create receiver from URI
    let receiver = match Receiver::try_from(uri) {
        Ok(receiver) => receiver,
        Err(e) => {
            log::error!("Failed to create receiver: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to create receiver: {}", e)
                })),
            )
                .into_response();
        }
    };

    let identifiers = Identifiers::from(payload).default_user(username.as_ref());
    let exporter = match Exporter::try_from(None) {
        Ok(exporter) => exporter.with_identifiers(identifiers),
        Err(e) => {
            log::error!("Failed to create exporter: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to create exporter: {}", e)
                })),
            )
                .into_response();
        }
    };

    let job = JobNew::new(&exporter.identifiers(), receiver);
    let job_id = job.id.clone();

    let job_ready = match job.ready(exporter).await {
        Ok(job_ready) => job_ready,
        Err(failed_job) => {
            log::error!("Failed to prepare job: {}", failed_job.error);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to prepare job: {}", failed_job.error)
                })),
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
        Json(serde_json::json!({
            "status": "processing",
            "job_id": job_id,
            "queue_size": queue_size,
        })),
    )
        .into_response()
}
