use super::{ServerState, get_user_email, template};
use askama::Template;
use axum::{
    http::HeaderMap,
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    FileUpload,
    ServiceLink,
    ApiKey,
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tab::FileUpload => write!(f, "file_upload"),
            Tab::ServiceLink => write!(f, "service_link"),
            Tab::ApiKey => write!(f, "api_key"),
        }
    }
}

pub async fn handler(headers: HeaderMap, state: Arc<ServerState>) -> impl IntoResponse {
    let (user_initial, user_email) = match get_user_email(&headers) {
        Some(email) => (email.chars().next().unwrap_or('_'), email),
        None => ('_', "Anonymous".to_string()),
    };
    let exporter_target = { state.exporter.read().await.to_string() };
    let index_html = template::Index {
        exporter: exporter_target,
        kibana_url: state.kibana.clone(),
        debug: log::max_level() == log::Level::Debug,
        user_initial,
        user: user_email,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let index_html = match index_html.render() {
        Ok(html) => html,
        Err(err) => format!(
            "<html><body><h1>Internal Server Error</h1><p>{}</p></body></html>",
            err
        ),
    };

    Html(index_html)
}
