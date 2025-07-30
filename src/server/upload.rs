use super::{ServerState, get_iap_email, template};
use crate::{data::diagnostic::report::Identifiers, processor::JobNew, receiver::Receiver};
use askama::Template;
use async_stream::stream;
use axum::{
    extract::Multipart,
    http::HeaderMap,
    response::{IntoResponse, Sse},
};
use datastar::{
    consts::ElementPatchMode,
    prelude::{PatchElements, PatchSignals},
};
use std::{convert::Infallible, sync::Arc};

pub async fn handler(
    headers: HeaderMap,
    mut multipart: Multipart,
    state: Arc<ServerState>,
) -> impl IntoResponse {
    // Extract authenticated user email from header
    let user_email = get_iap_email(&headers);

    // (
    //     StatusCode::BAD_REQUEST,
    //     Html(format!(
    //         r#"<div class="status-box error">
    //             🛑 <b>Error:</b> No file part in the request
    //         </div>"#
    //     )),
    // )
    // .into_response()
    //

    Sse::new(stream! {
        let signal = r#"{"uploading":true}"#;
        let sse_event = PatchSignals::new(signal).write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event);

        // Process the multipart form
        while let Ok(Some(field)) = multipart.next_field().await {
            if field.name() == Some("file") {
                // Check if the file has a valid filename
                let filename = match field.file_name() {
                    Some(filename) if !filename.ends_with(".zip") => {
                        let element = template::Error::new(
                            "error-file-type",
                            "Invalid file type",
                            "Only <code>.zip</code> files are allowed."
                        );
                        let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                        yield Ok::<_, Infallible>(sse_event);
                        filename.to_string()
                    }
                    Some(filename) => filename.to_string(),
                    None => {
                        let element = template::Error::new(
                            "error-file-name",
                            "Missing file name",
                            "No file name provided"
                        );
                        let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                        yield Ok::<_, Infallible>(sse_event);
                        "".to_string()
                    }
                };
                // Get the file data
                match field.bytes().await {
                    Ok(data) => {
                        let message = format!("Received upload: {} ({} bytes)", filename, data.len());
                        log::info!("{}", message);

                        let identifiers = Identifiers {
                            account: None,
                            case_number: None,
                            filename: Some(filename.clone()),
                            user: user_email.clone(),
                            opportunity: None,
                        };

                        let receiver = match Receiver::try_from(data) {
                            Ok(receiver) => receiver,
                            Err(e) => {
                                let error = format!("Failed to create receiver: {}", e);
                                log::error!("{}", error);
                                let element = template::Error::new(
                                    "error-receiver",
                                    "Failed to create upload receiver",
                                    &error
                                );
                                let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                                yield Ok::<_, Infallible>(sse_event);
                                break;
                            }
                        };

                        let element = template::Status::new("processing", &message);
                        let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                        yield Ok::<_, Infallible>(sse_event);

                        let exporter = {
                            state.exporter.read().await.clone().with_identifiers(identifiers)
                        };

                        let elements = match JobNew::new(&exporter.identifiers(), receiver).ready(exporter).await {
                            Ok(job) => {
                                match job.start().process().await {
                                    Ok(job) => {
                                       template::JobCompleted {
                                           diagnostic_id: &job.id,
                                           docs_created: &job.report.docs.created,
                                           filename: &job.filename,
                                           kibana_link: job.report.kibana_link.as_ref().unwrap_or(&"#".to_string()),
                                           product: &job.report.product.to_string(),
                                       }.render()
                                       .unwrap_or(template::Error::new("error", "Render error", "Failed to render template"))
                                    },
                                    Err(job) => {
                                        let element = template::Status::new("error", &job.error);
                                        state.job.record_failure(job).await;
                                        element
                                    },
                                }
                            },
                            Err(job) => {
                                let element = template::Status::new("error", &job.error);
                                state.job.record_failure(job).await;
                                element
                            },
                        };

                        let sse_event = PatchElements::new(elements).selector("#job-feed")
                            .mode(ElementPatchMode::After).write_as_axum_sse_event();
                        yield Ok::<_, Infallible>(sse_event);
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to read upload data: {}", e);
                        let element = template::Status::new("error", &error_msg);
                        let sse_event = PatchElements::new(element).write_as_axum_sse_event();
                        yield Ok::<_, Infallible>(sse_event);
                        log::error!("{}", error_msg);
                    }
                }
            }
        }
        let signal = r#"{"uploading":false}"#;
        let sse_event = PatchSignals::new(signal).write_as_axum_sse_event();
        yield Ok::<_, Infallible>(sse_event)
    })
}
