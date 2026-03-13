// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::{
    Identifiers, ServerEvent, ServerState, Signals, job_feed_event, receiver_stream, signal_event,
    template, workflow,
};
use crate::{data::KnownHost, processor::new_job_id};
use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Sse},
};
use datastar::axum::ReadSignals;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn form(
    State(state): State<Arc<ServerState>>,
    headers: HeaderMap,
    ReadSignals(signals): ReadSignals<Signals>,
) -> impl IntoResponse {
    let source = signals.workflow.collect.known_host.clone();
    let (tx, rx) = mpsc::channel(64);
    match state.resolve_user_email(&headers) {
        Ok((_, request_user)) => {
            tokio::spawn(async move {
                run_known_host_form(state, signals, request_user, tx).await;
            });
        }
        Err(err) => {
            tokio::spawn(async move {
                state.record_failure().await;
                send_event(
                    &tx,
                    job_feed_event(template::JobFailed {
                        job_id: new_job_id(),
                        error: &format!("Unauthorized request: {}", err),
                        source: &source,
                    }),
                )
                .await;
                send_event(&tx, signal_event(r#"{"loading":false}"#)).await;
            });
        }
    }
    Sse::new(receiver_stream(rx))
}

async fn send_event(tx: &mpsc::Sender<ServerEvent>, event: ServerEvent) {
    let _ = tx.send(event).await;
}

async fn run_known_host_form(
    state: Arc<ServerState>,
    signals: Signals,
    request_user: String,
    tx: mpsc::Sender<ServerEvent>,
) {
    let Some(host) = KnownHost::get_known(&signals.workflow.collect.known_host) else {
        state.record_failure().await;
        send_event(
            &tx,
            job_feed_event(template::JobFailed {
                job_id: new_job_id(),
                error: "Known host not found",
                source: &signals.workflow.collect.known_host,
            }),
        )
        .await;
        send_event(&tx, signal_event(r#"{"loading":false}"#)).await;
        return;
    };

    let job = super::WorkflowJob {
        identifiers: Identifiers::default(),
        artifact: super::CollectedArtifact::RemoteCollection {
            source: host.get_url().to_string(),
            host,
            diagnostic_type: signals.workflow.collect.diagnostic_type.clone(),
        },
    };

    workflow::run_job(state, signals, new_job_id(), request_user, tx, job).await;
}
