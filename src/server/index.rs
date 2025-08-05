use super::{ServerState, get_user_email, template};
use askama::Template;
use axum::{
    extract::Query,
    http::HeaderMap,
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{str::FromStr, sync::Arc};

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

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Params {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    link_id: Option<u64>,
    upload_id: Option<u64>,
}

/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

pub async fn handler(
    Query(params): Query<Params>,
    headers: HeaderMap,
    state: Arc<ServerState>,
) -> impl IntoResponse {
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
        link_id: params.link_id,
        upload_id: params.upload_id,
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
