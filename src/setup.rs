use crate::{exporter::Exporter, receiver::file};
use color_eyre::eyre::{eyre, Result};
use serde::Deserialize;
use serde_json::{from_slice, Value};
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct Asset {
    pub endpoint: String,
    //pub file: Option<String>,
    pub method: String,
    pub name: String,
    pub subdir: Option<String>,
    pub suffix: Option<String>,
}

pub async fn assets(exporter: Exporter) -> Result<()> {
    match exporter {
        Exporter::File(_) | Exporter::Stream(_) => {
            return Err(eyre!("Setup only supports Elasticsearch."))
        }
        _ => {}
    }

    // load asset list from ./assets/{product}/assets.yml
    let assets = file::parse_assets_yml(&exporter)?;

    for asset in assets {
        log::info!("Processing asset: {}", &asset.name);
        let dir_str = format!(
            "{}/{}",
            &exporter.as_str(),
            &asset.subdir.unwrap_or("".to_string())
        );
        let subdir = PathBuf::from(dir_str);
        let files = match file::ASSETS_DIR.get_dir(&subdir) {
            Some(dir) => dir.files(),
            None => return Err(eyre!("No assets directory found")),
        };

        // send assets to Elasticsearch
        match exporter {
            Exporter::Elasticsearch(ref exporter) => {
                // for each asset, send to Elasticsearch
                for file in files {
                    log::debug!("file.path: {:?}", &file.path());
                    let value: Option<Value> = match from_slice(file.contents()) {
                        Ok(value) => Some(value),
                        Err(e) => {
                            log::warn!("Failed to parse asset: {:?}", &e);
                            None
                        }
                    };
                    let stem = file.path().file_stem().unwrap().to_str().unwrap_or("");
                    let endpoint = format!(
                        "{}/{}{}",
                        &asset.endpoint,
                        &stem,
                        asset.suffix.clone().unwrap_or("".to_string()),
                    );
                    match exporter
                        .send(&asset.method, &endpoint, value.as_ref())
                        .await
                    {
                        Ok(response) => match response.status_code().is_success() {
                            true => {
                                log::info!(
                                    "{} {} {} {}",
                                    &asset.name,
                                    &stem,
                                    &asset.method,
                                    response.status_code()
                                )
                            }
                            false => {
                                let body = response.json::<Value>().await?;
                                log::error!("Asset sent ERROR: {body}");
                            }
                        },
                        Err(e) => log::error!("Failed to send asset: {e:?}"),
                    }
                }
            }
            _ => return Err(eyre!("Output target not supported")),
        }
    }
    Ok(())
}
