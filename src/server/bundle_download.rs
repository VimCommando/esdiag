// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::ServerState;
use axum::{
    extract::{Path, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    },
    response::IntoResponse,
};
use std::{sync::Arc, time::Duration};
use tokio::time::{Instant, sleep};

const RETAINED_BUNDLE_POST_DOWNLOAD_TTL: Duration = Duration::from_secs(300);
const RETAINED_BUNDLE_WAIT_TIMEOUT: Duration = Duration::from_secs(1800);
const RETAINED_BUNDLE_WAIT_POLL: Duration = Duration::from_millis(200);

pub async fn download_retained_bundle(
    State(state): State<Arc<ServerState>>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let (_, request_user) = match state.resolve_user_email(&headers) {
        Ok(result) => result,
        Err(err) => return (StatusCode::UNAUTHORIZED, err.to_string()).into_response(),
    };

    state
        .ensure_retained_bundle_token(
            &token,
            request_user.clone(),
            RETAINED_BUNDLE_WAIT_TIMEOUT,
        )
        .await;

    let Some(bundle) = wait_for_retained_bundle_resolution(&state, &token).await else {
        return (StatusCode::NOT_FOUND, "Download not ready").into_response();
    };

    if bundle.owner != request_user {
        return (StatusCode::FORBIDDEN, "Download does not belong to this user").into_response();
    }

    if let Some(error) = bundle.error {
        state.discard_retained_bundle(&token).await;
        return (StatusCode::CONFLICT, error).into_response();
    }

    if bundle.expires_at_epoch <= super::now_epoch_seconds() {
        state.discard_retained_bundle(&token).await;
        return (StatusCode::GONE, "Download has expired").into_response();
    }

    let Some(path) = bundle.path else {
        state.discard_retained_bundle(&token).await;
        return (StatusCode::NOT_FOUND, "Download bundle missing").into_response();
    };
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            state.discard_retained_bundle(&token).await;
            return (
                StatusCode::NOT_FOUND,
                format!("Download is no longer available: {err}"),
            )
                .into_response();
        }
    };

    let _ = state
        .touch_retained_bundle(&token, RETAINED_BUNDLE_POST_DOWNLOAD_TTL)
        .await;
    state.schedule_retained_bundle_cleanup(token, RETAINED_BUNDLE_POST_DOWNLOAD_TTL);

    let safe_filename = bundle
        .filename
        .unwrap_or_else(|| "diagnostic.zip".to_string())
        .replace('"', "_");
    let disposition = format!("attachment; filename=\"{safe_filename}\"");

    let content_length = bytes.len();
    let mut response = bytes.into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&content_length.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response
}

async fn wait_for_retained_bundle_resolution(
    state: &Arc<ServerState>,
    token: &str,
) -> Option<super::RetainedBundle> {
    let deadline = Instant::now() + RETAINED_BUNDLE_WAIT_TIMEOUT;
    loop {
        if let Some(bundle) = state.retained_bundle(token).await
            && (bundle.error.is_some() || bundle.path.is_some())
        {
            return Some(bundle);
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(RETAINED_BUNDLE_WAIT_POLL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::download_retained_bundle;
    use crate::server::{RetainedBundle, now_epoch_seconds, test_server_state};
    use axum::{
        body::to_bytes,
        extract::{Path, State},
        http::{HeaderMap, StatusCode, header::CONTENT_DISPOSITION},
        response::IntoResponse,
    };
    use std::time::Duration;

    #[tokio::test]
    async fn download_retained_bundle_returns_zip_attachment() {
        let state = test_server_state();
        let path = std::env::temp_dir().join("esdiag-retained-bundle-test.zip");
        tokio::fs::write(&path, b"zip-bytes")
            .await
            .expect("write retained bundle");

        let token = state
            .insert_retained_bundle(
                "Anonymous".to_string(),
                "diagnostic.zip".to_string(),
                path,
                Duration::from_secs(60),
            )
            .await;

        let response =
            download_retained_bundle(State(state), Path(token), HeaderMap::new())
                .await
                .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_DISPOSITION).unwrap(),
            "attachment; filename=\"diagnostic.zip\""
        );
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        assert_eq!(&body[..], b"zip-bytes");
    }

    #[tokio::test]
    async fn download_retained_bundle_discards_expired_entries() {
        let state = test_server_state();
        let token = "expired-token".to_string();
        let path = std::env::temp_dir().join("esdiag-retained-expired-bundle-test.zip");
        tokio::fs::write(&path, b"expired")
            .await
            .expect("write expired bundle");
        state.retained_bundles.write().await.insert(
            token.clone(),
            RetainedBundle {
                owner: "Anonymous".to_string(),
                accepted: true,
                error: None,
                filename: Some("expired.zip".to_string()),
                path: Some(path.clone()),
                expires_at_epoch: now_epoch_seconds() - 1,
            },
        );

        let response =
            download_retained_bundle(State(state.clone()), Path(token.clone()), HeaderMap::new())
                .await
                .into_response();

        assert_eq!(response.status(), StatusCode::GONE);
        assert!(state.retained_bundle(&token).await.is_none());
        assert!(!path.exists(), "expired bundle file should be removed");
    }
}
