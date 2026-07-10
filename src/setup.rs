// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use crate::{
    client::Client,
    data::Product,
    embeds::{Assets, KIBANA_ASSETS_BUNDLE},
};
//use bytes::Bytes;
use eyre::{Result, WrapErr, eyre};
#[cfg(test)]
use kibana_sync::kibana::saved_objects::SavedObjectsManifest;
use kibana_sync::kibana::spaces::{SpaceEntry, SpacesManifest};
use kibana_sync::{
    KibanaFsBundle,
    sync::{SyncOptions, push_sync},
};
use regex::Regex;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tempfile::TempDir;
use zip::ZipArchive;

// Subdirectory for templates and configs files
pub static ASSETS_FILE: &str = "assets.yml";
pub static SOURCES_FILE: &str = "sources.yml";
const KIBANA_ASSETS_DIR: &str = "kibana";
const KIBANA_SPACES_FILE: &str = "spaces.yml";
const KIBANA_SPACE_DEFINITION_FILE: &str = "space.json";
#[cfg(test)]
const KIBANA_MANIFEST_DIR: &str = "manifest";
#[cfg(test)]
const KIBANA_SAVED_OBJECTS_MANIFEST: &str = "saved_objects.json";
const DEFAULT_AGENT_BUILDER_AGENT_ID: &str = "elastic-ai-agent";
static JSON5_TRIPLE_QUOTED_STRINGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)"{3}(.*?)"{3}"#).expect("valid JSON5 triple-quoted string pattern"));

struct EmbeddedAssets;

impl EmbeddedAssets {
    fn new() -> Result<Self> {
        Ok(Self)
    }

    fn get_file(&self, path: &Path) -> Option<std::borrow::Cow<'static, [u8]>> {
        if let Some(path_str) = path.to_str() {
            if path_str.starts_with(KIBANA_ASSETS_DIR) {
                return get_kibana_bundle_file(path_str);
            }
            Assets::get(path_str).map(|f| f.data)
        } else {
            None
        }
    }

    fn get_dir_files(&self, path: &Path) -> Vec<(PathBuf, std::borrow::Cow<'static, [u8]>)> {
        let prefix = path.to_str().unwrap_or("");
        if prefix.starts_with(KIBANA_ASSETS_DIR) {
            return get_kibana_bundle_dir_files(prefix);
        }

        let mut files: Vec<_> = Assets::iter()
            .filter(|p| p.starts_with(prefix))
            .filter_map(|p| {
                let p_str = p.as_ref();
                let p_buf = PathBuf::from(p_str);
                Assets::get(p_str).map(|f| (p_buf, f.data))
            })
            .collect();
        files.sort_by(|(p1, _), (p2, _)| p1.cmp(p2));
        files
    }
}

fn open_kibana_bundle() -> Option<ZipArchive<Cursor<&'static [u8]>>> {
    ZipArchive::new(Cursor::new(KIBANA_ASSETS_BUNDLE)).ok()
}

fn get_kibana_bundle_file(path: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    let mut archive = open_kibana_bundle()?;
    let mut file = archive.by_name(path).ok()?;
    let mut contents = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut contents).ok()?;
    Some(std::borrow::Cow::Owned(contents))
}

fn get_kibana_bundle_dir_files(prefix: &str) -> Vec<(PathBuf, std::borrow::Cow<'static, [u8]>)> {
    let Some(mut archive) = open_kibana_bundle() else {
        return Vec::new();
    };

    let mut files = Vec::new();
    for i in 0..archive.len() {
        let Ok(mut file) = archive.by_index(i) else {
            continue;
        };
        if !file.is_file() || !file.name().starts_with(prefix) {
            continue;
        }

        let mut contents = Vec::with_capacity(file.size() as usize);
        if file.read_to_end(&mut contents).is_ok() {
            files.push((PathBuf::from(file.name()), std::borrow::Cow::Owned(contents)));
        }
    }

    files.sort_by(|(p1, _), (p2, _)| p1.cmp(p2));
    files
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Asset {
    pub endpoint: String,
    pub method: String,
    pub name: String,
    #[serde(default = "default_headers")]
    pub headers: HashMap<String, String>,
    pub suffix: Option<String>,
    pub query: Option<String>,
    #[serde(default)]
    pub requires_security: bool,
}

fn default_headers() -> HashMap<String, String> {
    HashMap::from([("Content-Type".to_string(), "application/json".to_string())])
}

fn should_skip_asset(asset: &Asset, security_enabled: bool) -> bool {
    asset.requires_security && !security_enabled
}

async fn send_asset(client: &Client, asset: &Asset, path: &Path, contents: &[u8], named: bool) -> Result<()> {
    send_asset_with_allowed_statuses(client, asset, path, contents, named, &[]).await
}

async fn send_asset_with_allowed_statuses(
    client: &Client,
    asset: &Asset,
    path: &Path,
    contents: &[u8],
    named: bool,
    allowed_statuses: &[StatusCode],
) -> Result<()> {
    let stem = path.file_stem().unwrap().to_str().unwrap_or("");
    let endpoint = match named {
        true => &format!(
            "{}/{}{}",
            &asset.endpoint,
            &stem,
            asset.suffix.clone().unwrap_or("".to_string()),
        ),
        false => &asset.endpoint,
    };
    match client
        .request(asset.method.parse()?, &asset.headers, endpoint, Some(contents))
        .await
    {
        Ok(response) => {
            let status = response.status();
            match status.is_success() || allowed_statuses.contains(&status) {
                true => {
                    let body = response.text().await?;
                    tracing::info!("{} {} {} {}", &asset.name, &stem, &asset.method, status);
                    tracing::trace!("Response body: {}", body);
                    Ok(())
                }
                false => {
                    let bytes = response.bytes().await?;
                    let body = serde_json::from_slice::<Value>(&bytes)?;
                    let message = format!("Asset: {body}");
                    Err(eyre!(message))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to send asset: {e:?}");
            Err(eyre!(e))
        }
    }
}

/// Submit saved assets to the client APIs
pub async fn assets(client: &Client) -> Result<()> {
    let embedded_assets = EmbeddedAssets::new()?;
    if Product::from(client) == Product::Kibana {
        return kibana_assets(client, &embedded_assets).await;
    }

    // load asset list from ./assets/{product}/assets.yml
    let assets = parse_assets_yml(client.into(), &embedded_assets)?;

    // Check security status
    let security_enabled = client
        .has_security_enabled()
        .await
        .wrap_err("Failed to determine security status")?;

    if !security_enabled {
        tracing::info!("Security is disabled on the cluster. Security-dependent assets will be skipped.");
    }

    let mut error_count = 0;

    for asset in assets {
        if should_skip_asset(&asset, security_enabled) {
            tracing::debug!("Skipping security-dependent asset: {}", &asset.name);
            continue;
        }

        tracing::info!("Processing asset: {}", &asset.name);
        tracing::debug!("Asset: {:?}", &asset);
        let path = PathBuf::from(format!("{}/{}", client, asset.name));

        let dir_files = embedded_assets.get_dir_files(&path);
        if !dir_files.is_empty() {
            // do something with the directory
            for (file_path, contents) in dir_files {
                tracing::debug!("file.path: {:?}", file_path);
                match send_asset(client, &asset, &file_path, &contents, true).await {
                    Ok(res) => tracing::debug!("Response: {:?}", res),
                    Err(e) => {
                        tracing::error!("Failed to send asset: {e:?}");
                        error_count += 1;
                    }
                }
            }
        } else if let Some(contents) = embedded_assets.get_file(&path) {
            // do something with the file
            tracing::debug!("file.path: {:?}", &path);
            if let Err(e) = send_asset(client, &asset, &path, &contents, false).await {
                tracing::error!("Failed to send asset: {e:?}");
                error_count += 1;
            }
        } else {
            tracing::error!("Asset not found: {}", &asset.name);
            return Err(eyre!("Asset not found: {}", asset.name));
        }
    }
    if error_count == 0 {
        tracing::info!("completed setup for {client}");
        Ok(())
    } else {
        tracing::error!("{error_count} errors in setup for {client}");
        Err(eyre!("{error_count} errors in setup for {client}"))
    }
}

async fn kibana_assets(client: &Client, embedded_assets: &EmbeddedAssets) -> Result<()> {
    let spaces_manifest = parse_kibana_spaces_yml(embedded_assets)?;
    let mut error_count = 0;

    for space in &spaces_manifest.spaces {
        let space_payload = kibana_space_payload(space, embedded_assets)?;
        let space_asset = Asset {
            endpoint: "api/spaces/space".to_string(),
            method: Method::POST.to_string(),
            name: KIBANA_SPACE_DEFINITION_FILE.to_string(),
            headers: default_headers(),
            suffix: None,
            query: None,
            requires_security: false,
        };
        let space_path = kibana_space_definition_path(&space.id);
        if let Err(e) = send_asset_with_allowed_statuses(
            client,
            &space_asset,
            &space_path,
            &space_payload,
            false,
            &[StatusCode::CONFLICT],
        )
        .await
        {
            tracing::error!("Failed to send Kibana space asset: {e:?}");
            error_count += 1;
        }
    }

    if let Err(e) = kibana_bundle_assets(client).await {
        tracing::error!("Failed to send Kibana bundled assets: {e:?}");
        error_count += 1;
    }

    if error_count == 0 {
        tracing::info!("completed setup for {client}");
        Ok(())
    } else {
        tracing::error!("{error_count} errors in setup for {client}");
        Err(eyre!("{error_count} errors in setup for {client}"))
    }
}

pub async fn ensure_enterprise_license(client: &Client) -> Result<()> {
    let license = get_json_response(client, Method::GET, "_license").await?;
    let license_type = license
        .pointer("/license/type")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("Elasticsearch did not return a license type"))?;

    if matches!(license_type, "enterprise" | "trial") {
        tracing::info!("Enterprise features are available through the {license_type} license");
        return Ok(());
    }

    let trial_status = get_json_response(client, Method::GET, "_license/trial_status").await?;
    let eligible = trial_status
        .get("eligible_to_start_trial")
        .and_then(Value::as_bool)
        .ok_or_else(|| eyre!("Elasticsearch did not return trial eligibility"))?;
    if !eligible {
        return Err(eyre!(
            "Enterprise features are required for Kibana Agent Builder assets, but this cluster's trial has already been used"
        ));
    }

    let response = client
        .request(
            Method::POST,
            &default_headers(),
            "_license/start_trial?acknowledge=true",
            None,
        )
        .await
        .wrap_err("Failed to start the Elasticsearch Enterprise trial")?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .wrap_err("Failed to read Elasticsearch trial activation response")?;
    if !status.is_success() {
        return Err(eyre!("Failed to start the Elasticsearch Enterprise trial: {body}"));
    }
    if body.get("trial_was_started").and_then(Value::as_bool) != Some(true) {
        return Err(eyre!(
            "Elasticsearch did not start the Enterprise trial: {}",
            body.get("error_message")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        ));
    }

    tracing::info!("Started the 30-day Elasticsearch Enterprise trial");
    Ok(())
}

async fn get_json_response(client: &Client, method: Method, path: &str) -> Result<Value> {
    let response = client.request(method, &HashMap::new(), path, None).await?;
    let status = response.status();
    let body: Value = response.json().await?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(eyre!("Request to {path} failed with {status}: {body}"))
    }
}

async fn kibana_bundle_assets(client: &Client) -> Result<()> {
    let mut bundle = read_embedded_kibana_sync_bundle()?;
    // Space definitions carry additional ESDiag settings that the generic
    // filesystem manifest does not represent, so they are created above.
    bundle.spaces.clear();

    let Client::Kibana(kibana_client) = client else {
        return Err(eyre!("Kibana Agent Builder assets require a Kibana client"));
    };
    let summary = push_sync(kibana_client.inner(), &bundle, &SyncOptions::default())
        .await
        .map_err(|error| eyre!(error))?;

    let expected_saved_objects = bundle
        .by_space
        .values()
        .map(|space| space.saved_objects.len())
        .sum::<usize>();
    let expected_tools = bundle.by_space.values().map(|space| space.tools.len()).sum::<usize>();
    let expected_skills = bundle.by_space.values().map(|space| space.skills.len()).sum::<usize>();
    let expected_workflows = bundle
        .by_space
        .values()
        .map(|space| space.workflows.len())
        .sum::<usize>();
    if (
        summary.saved_objects_applied,
        summary.tools_applied,
        summary.skills_applied,
        summary.workflows_applied,
    ) != (
        expected_saved_objects,
        expected_tools,
        expected_skills,
        expected_workflows,
    ) {
        return Err(eyre!(
            "Kibana did not apply all bundled assets (saved objects {}/{}, tools {}/{}, skills {}/{}, workflows {}/{})",
            summary.saved_objects_applied,
            expected_saved_objects,
            summary.tools_applied,
            expected_tools,
            summary.skills_applied,
            expected_skills,
            summary.workflows_applied,
            expected_workflows
        ));
    }

    for (space_id, space) in bundle.by_space {
        let skill_ids = space
            .skills
            .iter()
            .filter_map(|skill| skill.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        if !skill_ids.is_empty() {
            associate_skills_with_default_agent(client, &space_id, &skill_ids).await?;
        }
    }
    Ok(())
}

fn extract_kibana_bundle() -> Result<TempDir> {
    let temporary_bundle = TempDir::new().wrap_err("Failed to create temporary Kibana asset directory")?;
    let mut archive = open_kibana_bundle().ok_or_else(|| eyre!("Failed to open embedded Kibana asset bundle"))?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let path = file
            .enclosed_name()
            .ok_or_else(|| eyre!("Kibana asset bundle contains an invalid path"))?
            .to_path_buf();
        let destination = temporary_bundle.path().join(&path);
        if file.is_dir() {
            std::fs::create_dir_all(destination)?;
            continue;
        }
        let parent = destination
            .parent()
            .ok_or_else(|| eyre!("Kibana asset bundle file has no parent directory"))?;
        std::fs::create_dir_all(parent)?;
        let mut contents = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut contents)?;
        if destination.extension().and_then(|extension| extension.to_str()) == Some("json") {
            contents = normalize_kibana_asset_json5(&contents, &path)?;
        }
        let mut output = std::fs::File::create(destination)?;
        output.write_all(&contents)?;
        output.flush()?;
    }
    Ok(temporary_bundle)
}

fn read_embedded_kibana_sync_bundle() -> Result<kibana_sync::sync::SyncBundle> {
    let temporary_bundle = extract_kibana_bundle()?;
    KibanaFsBundle::open(temporary_bundle.path().join(KIBANA_ASSETS_DIR))
        .map_err(|error| eyre!("{error}"))?
        .read_all()
        .map_err(|error| eyre!("{error}"))
}

fn normalize_kibana_asset_json5(contents: &[u8], path: &Path) -> Result<Vec<u8>> {
    let source = std::str::from_utf8(contents)
        .wrap_err_with(|| format!("Kibana JSON5 asset is not UTF-8: {}", path.display()))?;
    let triple_quote_count = source.matches("\"\"\"").count();
    if !triple_quote_count.is_multiple_of(2) {
        return Err(eyre!(
            "Kibana JSON5 asset has an unmatched triple-quoted string: {}",
            path.display()
        ));
    }
    Ok(JSON5_TRIPLE_QUOTED_STRINGS
        .replace_all(source, |captures: &regex::Captures| {
            serde_json::to_string(&captures[1]).unwrap()
        })
        .into_owned()
        .into_bytes())
}

async fn associate_skills_with_default_agent(client: &Client, space_id: &str, skill_ids: &[&str]) -> Result<()> {
    let agent_path = format!("s/{space_id}/api/agent_builder/agents/{DEFAULT_AGENT_BUILDER_AGENT_ID}");
    let mut agent = get_json_response(client, Method::GET, &agent_path)
        .await
        .wrap_err_with(|| {
            format!(
                "Failed to load the default Agent Builder agent in Kibana space '{space_id}'. Open Agent Builder in that space once, then rerun setup"
            )
        })?;
    if agent.get("readonly").and_then(Value::as_bool) == Some(true) {
        return Err(eyre!(
            "The default Agent Builder agent in Kibana space '{space_id}' is read-only and cannot be assigned ESDiag skills"
        ));
    }

    let agent_object = agent
        .as_object_mut()
        .ok_or_else(|| eyre!("Kibana returned an invalid default Agent Builder agent"))?;
    for field in [
        "id",
        "readonly",
        "schema",
        "type",
        "created_at",
        "created_by",
        "updated_at",
        "updated_by",
    ] {
        agent_object.remove(field);
    }
    let configuration = agent_object
        .entry("configuration")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| eyre!("Kibana returned an invalid Agent Builder configuration"))?;
    let assigned_skills = configuration
        .entry("skill_ids")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| eyre!("Kibana returned invalid Agent Builder skill assignments"))?;
    for skill_id in skill_ids {
        if !assigned_skills.iter().any(|value| value.as_str() == Some(*skill_id)) {
            assigned_skills.push(json!(skill_id));
        }
    }

    let response = client
        .request(
            Method::PUT,
            &default_headers(),
            &agent_path,
            Some(&serde_json::to_vec(&agent)?),
        )
        .await?;
    let status = response.status();
    if status.is_success() {
        tracing::info!("Associated ESDiag skills with the default Agent Builder agent in space '{space_id}'");
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(eyre!(
            "Failed to associate ESDiag skills with the default Agent Builder agent in space '{space_id}': {status} {body}"
        ))
    }
}

/// Parses the assets YAML file for the given exporter. Currently only supports Elasticsearch.
fn parse_assets_yml(product: Product, assets_store: &EmbeddedAssets) -> Result<Vec<Asset>> {
    let filename = format!("{}/{}", product.to_string().to_lowercase(), ASSETS_FILE);
    let contents = assets_store
        .get_file(Path::new(&filename))
        .ok_or(eyre!("embedded assets did not contain expected file {filename}"))?;
    let assets = serde_yaml::from_slice(&contents)?;
    Ok(assets)
}

fn parse_kibana_spaces_yml(assets_store: &EmbeddedAssets) -> Result<SpacesManifest> {
    let filename = PathBuf::from(KIBANA_ASSETS_DIR).join(KIBANA_SPACES_FILE);
    let contents = assets_store.get_file(&filename).ok_or(eyre!(
        "embedded assets did not contain expected file {}",
        filename.display()
    ))?;
    let manifest = serde_yaml::from_slice(&contents)?;
    Ok(manifest)
}

#[cfg(test)]
fn parse_kibana_saved_objects_manifest(space_id: &str, assets_store: &EmbeddedAssets) -> Result<SavedObjectsManifest> {
    let filename = kibana_saved_objects_manifest_path(space_id);
    let contents = assets_store.get_file(&filename).ok_or(eyre!(
        "embedded assets did not contain expected file {}",
        filename.display()
    ))?;
    let manifest = serde_json::from_slice(&contents)?;
    Ok(manifest)
}

fn kibana_space_payload(space: &SpaceEntry, assets_store: &EmbeddedAssets) -> Result<Vec<u8>> {
    let path = kibana_space_definition_path(&space.id);
    if let Some(contents) = assets_store.get_file(&path) {
        return Ok(contents.into_owned());
    }

    serde_json::to_vec(&json!({
        "id": space.id,
        "name": space.name,
    }))
    .map_err(Into::into)
}

fn kibana_space_definition_path(space_id: &str) -> PathBuf {
    PathBuf::from(KIBANA_ASSETS_DIR)
        .join(space_id)
        .join(KIBANA_SPACE_DEFINITION_FILE)
}

#[cfg(test)]
fn kibana_saved_objects_manifest_path(space_id: &str) -> PathBuf {
    PathBuf::from(KIBANA_ASSETS_DIR)
        .join(space_id)
        .join(KIBANA_MANIFEST_DIR)
        .join(KIBANA_SAVED_OBJECTS_MANIFEST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ElasticsearchBuilder;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use url::Url;

    #[test]
    fn test_asset_deserialization_with_requires_security() {
        let yaml = r#"
- name: "roles"
  endpoint: "_security/role"
  method: "PUT"
  requires_security: true
- name: "ingest_pipelines"
  endpoint: "_ingest/pipeline"
  method: "PUT"
"#;
        let assets: Vec<Asset> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].name, "roles");
        assert!(assets[0].requires_security);
        assert_eq!(assets[1].name, "ingest_pipelines");
        assert!(!assets[1].requires_security);
    }

    #[test]
    fn test_should_skip_asset() {
        let security_asset = Asset {
            endpoint: "/".to_string(),
            method: "GET".to_string(),
            name: "test".to_string(),
            headers: HashMap::new(),
            suffix: None,
            query: None,
            requires_security: true,
        };
        let normal_asset = Asset {
            endpoint: "/".to_string(),
            method: "GET".to_string(),
            name: "test".to_string(),
            headers: HashMap::new(),
            suffix: None,
            query: None,
            requires_security: false,
        };

        // Security enabled: skip nothing
        assert!(!should_skip_asset(&security_asset, true));
        assert!(!should_skip_asset(&normal_asset, true));

        // Security disabled: skip security asset
        assert!(should_skip_asset(&security_asset, false));
        assert!(!should_skip_asset(&normal_asset, false));
    }

    #[tokio::test]
    async fn enterprise_and_trial_licenses_do_not_start_a_trial() {
        for license_type in ["enterprise", "trial"] {
            let (client, server) =
                mock_elasticsearch(vec![format!(r#"{{"license":{{"type":"{license_type}"}}}}"#)]).await;

            ensure_enterprise_license(&client).await.unwrap();

            let requests = server.await.unwrap();
            assert_eq!(requests, vec!["GET /_license HTTP/1.1"]);
        }
    }

    #[tokio::test]
    async fn eligible_license_starts_enterprise_trial() {
        let (client, server) = mock_elasticsearch(vec![
            r#"{"license":{"type":"basic"}}"#.to_string(),
            r#"{"eligible_to_start_trial":true}"#.to_string(),
            r#"{"trial_was_started":true}"#.to_string(),
        ])
        .await;

        ensure_enterprise_license(&client).await.unwrap();

        let requests = server.await.unwrap();
        assert_eq!(
            requests,
            vec![
                "GET /_license HTTP/1.1",
                "GET /_license/trial_status HTTP/1.1",
                "POST /_license/start_trial?acknowledge=true HTTP/1.1",
            ]
        );
    }

    #[tokio::test]
    async fn ineligible_license_returns_error_without_starting_trial() {
        let (client, server) = mock_elasticsearch(vec![
            r#"{"license":{"type":"basic"}}"#.to_string(),
            r#"{"eligible_to_start_trial":false}"#.to_string(),
        ])
        .await;

        let error = ensure_enterprise_license(&client).await.unwrap_err();

        assert!(error.to_string().contains("trial has already been used"));
        let requests = server.await.unwrap();
        assert_eq!(
            requests,
            vec!["GET /_license HTTP/1.1", "GET /_license/trial_status HTTP/1.1",]
        );
    }

    #[test]
    fn kibana_assets_follow_kibana_sync_bundle_layout() {
        let bundle = read_embedded_kibana_sync_bundle().unwrap();

        assert_eq!(bundle.spaces.len(), 1);
        assert_eq!(bundle.spaces[0]["id"], "esdiag");

        let esdiag = bundle.by_space.get("esdiag").unwrap();
        assert_eq!(esdiag.saved_objects.len(), 90);
        assert_eq!(esdiag.workflows.len(), 1);
        assert!(esdiag.agents.is_empty());
        assert_eq!(esdiag.tools.len(), 1);
        assert_eq!(esdiag.skills.len(), 1);
    }

    #[test]
    fn kibana_assets_are_embedded_as_bundle_not_raw_files() {
        assert!(KIBANA_ASSETS_BUNDLE.len() > 0);
        assert!(Assets::get("kibana/spaces.yml").is_none());

        let embedded_assets = EmbeddedAssets::new().unwrap();
        let spaces = embedded_assets
            .get_file(Path::new("kibana/spaces.yml"))
            .expect("Kibana spaces manifest should load from bundle");

        assert!(std::str::from_utf8(&spaces).unwrap().contains("id: esdiag"));
    }

    #[test]
    fn kibana_space_payload_preserves_full_space_definition() {
        let embedded_assets = EmbeddedAssets::new().unwrap();
        let spaces = parse_kibana_spaces_yml(&embedded_assets).unwrap();
        let payload = kibana_space_payload(&spaces.spaces[0], &embedded_assets).unwrap();
        let value: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(value["id"], "esdiag");
        assert_eq!(value["description"], "Elastic Stack Diagnostics");
        assert_eq!(value["solution"], "oblt");
        assert!(value["disabledFeatures"].as_array().unwrap().contains(&json!("siemV5")));
    }

    #[test]
    fn kibana_saved_objects_ndjson_uses_manifest_order() {
        let manifest = parse_kibana_saved_objects_manifest("esdiag", &EmbeddedAssets::new().unwrap()).unwrap();
        let bundle = read_embedded_kibana_sync_bundle().unwrap();
        let saved_objects = &bundle.by_space["esdiag"].saved_objects;

        assert_eq!(saved_objects.len(), manifest.objects.len());
        assert_eq!(manifest.objects.len(), 90);
        for (object, expected) in saved_objects.iter().zip(manifest.objects) {
            assert_eq!(object["type"], expected.object_type);
            assert_eq!(object["id"], expected.id);
        }
    }

    #[test]
    fn kibana_saved_objects_have_valid_embedded_json_content() {
        let bundle = read_embedded_kibana_sync_bundle().unwrap();

        for object in &bundle.by_space["esdiag"].saved_objects {
            let label = saved_object_label(&object);
            let attributes = object
                .get("attributes")
                .unwrap_or_else(|| panic!("{label} should have attributes"));

            assert_json_string_fields_parse(&label, attributes);
            assert_vega_spec_parses(&label, attributes);
        }
    }

    #[test]
    fn kibana_readme_dashboard_links_to_esdiag_issues() {
        let bundle = read_embedded_kibana_sync_bundle().unwrap();
        let object = bundle.by_space["esdiag"]
            .saved_objects
            .iter()
            .find(|object| object["type"] == "dashboard" && object["id"] == "esdiag-readme")
            .expect("readme dashboard should be embedded");
        let dashboard = object.to_string();

        assert!(dashboard.contains("https://github.com/elastic/esdiag/issues"));
        assert!(!dashboard.contains("https://github.com/elastic/issues)"));
    }

    fn saved_object_label(object: &Value) -> String {
        format!(
            "{}/{}",
            object["type"].as_str().unwrap_or("<missing-type>"),
            object["id"].as_str().unwrap_or("<missing-id>")
        )
    }

    fn assert_json_string_fields_parse(label: &str, value: &Value) {
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    if let Some(text) = child.as_str()
                        && (key == "visState" || key.ends_with("JSON"))
                    {
                        serde_json::from_str::<Value>(text)
                            .unwrap_or_else(|err| panic!("{label}.{key} should parse as JSON: {err}"));
                    }
                    assert_json_string_fields_parse(label, child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    assert_json_string_fields_parse(label, child);
                }
            }
            _ => {}
        }
    }

    fn assert_vega_spec_parses(label: &str, attributes: &Value) {
        let Some(vis_state) = attributes.get("visState").and_then(Value::as_str) else {
            return;
        };
        let vis_state: Value = serde_json::from_str(vis_state)
            .unwrap_or_else(|err| panic!("{label}.visState should parse as JSON: {err}"));

        if vis_state["type"].as_str() != Some("vega") {
            return;
        }

        let spec = vis_state["params"]["spec"]
            .as_str()
            .unwrap_or_else(|| panic!("{label}.visState.params.spec should be a string"));
        serde_json::from_str::<Value>(spec)
            .unwrap_or_else(|err| panic!("{label}.visState.params.spec should parse as JSON: {err}"));
    }

    async fn mock_elasticsearch(responses: Vec<String>) -> (Client, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for body in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 1024];
                let bytes_read = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..bytes_read]);
                requests.push(request.lines().next().unwrap().to_string());
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
            requests
        });

        (
            Client::Elasticsearch(ElasticsearchBuilder::new(url).build().unwrap()),
            server,
        )
    }
}
