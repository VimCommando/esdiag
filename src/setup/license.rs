// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use crate::client::Client;
//use bytes::Bytes;
use eyre::{Result, eyre};
use reqwest::Method;
use serde_json::Value;
use std::collections::HashMap;

/// Start and verify a trial license before loading Enterprise-only Kibana assets.
pub async fn ensure_agent_builder_license(client: &Client) -> Result<()> {
    let Client::Elasticsearch(_) = client else {
        return Err(eyre!("an Elasticsearch client is required to start the trial license"));
    };

    if client.is_serverless().await? {
        tracing::info!("Serverless manages feature entitlements; skipping the Elasticsearch trial license API");
        return Ok(());
    }

    if agent_builder_license_is_active(&current_license(client).await?) {
        return Ok(());
    }

    tracing::info!("Starting Elasticsearch trial license for Kibana Agent Builder assets");
    let response = client
        .request(
            Method::POST,
            &HashMap::new(),
            "_license/start_trial?acknowledge=true",
            None,
        )
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(eyre!(
            "Failed to start Elasticsearch trial license ({status}): {}",
            response.text().await?
        ));
    }

    if agent_builder_license_is_active(&current_license(client).await?) {
        Ok(())
    } else {
        Err(eyre!(
            "Elasticsearch trial license did not become active; Kibana Agent Builder assets will not be loaded"
        ))
    }
}

pub(super) async fn current_license(client: &Client) -> Result<Value> {
    let response = client.request(Method::GET, &HashMap::new(), "_license", None).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(eyre!(
            "Failed to read Elasticsearch license ({status}): {}",
            response.text().await?
        ));
    }
    Ok(response.json().await?)
}

pub(super) fn agent_builder_license_is_active(response: &Value) -> bool {
    let license = response.get("license").unwrap_or(response);
    license.get("status").and_then(Value::as_str) == Some("active")
        && matches!(
            license.get("type").and_then(Value::as_str),
            Some("trial" | "enterprise")
        )
}
