extern crate elasticsearch as es_client;
pub mod elastic;
/// Export to an Elasticsearch cluster with the `_bulk` API
pub mod elasticsearch;
/// Write to an `.ndjson` file
pub mod file;
/// Write `ndjson` to std out
pub mod stream;

use crate::{
    client::Host,
    data::{diagnostic::Product, Uri},
};
use color_eyre::eyre::{eyre, Result};
use elastic::ElasticsearchExporter;
use elasticsearch::ElasticsearchClient;
use file::FileExporter;
use serde_json::Value;
use std::{fmt, path::PathBuf};
use stream::StreamExporter;
use url::Url;

pub trait Export {
    #[allow(async_fn_in_trait)]
    async fn is_connected(&self) -> bool;
    #[allow(async_fn_in_trait)]
    async fn write(&self, index: String, docs: Vec<Value>) -> Result<usize>;
}

pub enum Exporter {
    Elasticsearch(ElasticsearchExporter),
    File(FileExporter),
    Stream(StreamExporter),
}

impl Exporter {
    pub async fn write(&self, index: String, docs: Vec<Value>) -> Result<usize> {
        match self {
            Exporter::Elasticsearch(exporter) => exporter.write(index, docs).await,
            Exporter::File(exporter) => exporter.write(index, docs).await,
            Exporter::Stream(exporter) => exporter.write(index, docs).await,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Exporter::Elasticsearch(_) => "elasticsearch",
            Exporter::File(_) => "file",
            Exporter::Stream(_) => "stream",
        }
    }
}

impl TryFrom<Uri> for Exporter {
    type Error = color_eyre::Report;
    fn try_from(uri: Uri) -> std::result::Result<Self, Self::Error> {
        match uri {
            Uri::File(file) => Ok(Exporter::File(FileExporter::try_from(file)?)),
            Uri::Host(host) => Ok(Exporter::Elasticsearch(ElasticsearchExporter::try_from(
                host,
            )?)),
            Uri::Stream => Ok(Exporter::Stream(StreamExporter::new())),
            _ => Err(eyre!("Unsupported URI")),
        }
    }
}

#[derive(Debug)]
pub enum Target {
    Elasticsearch(ElasticsearchClient),
    File(PathBuf),
    Stdout,
}

pub struct Output {
    pub target: Target,
}

impl Output {
    pub fn new() -> Self {
        Self {
            target: Target::Stdout,
        }
    }

    pub async fn test(&self) -> Result<Value, Value> {
        let elasticsearch = match &self.target {
            Target::Elasticsearch(client) => client,
            _ => panic!("No Elasticsearch client"),
        };
        let response = match elasticsearch.test().await {
            Ok(response) => response,
            Err(e) => {
                log::error!("Failed to connect to Elasticsearch: {}", e);
                return Err(serde_json::json!({"error": e.to_string()}));
            }
        };
        match response.status_code().is_success() {
            true => {
                log::info!("Elasticsearch connection: {}", &response.status_code());
                Ok(response
                    .json::<Value>()
                    .await
                    .expect("Failed to parse response"))
            }
            false => {
                log::error!("Elasticsearch connection: {}", response.status_code());
                Err(response
                    .json::<Value>()
                    .await
                    .expect("Failed to parse response"))
            }
        }
    }

    pub fn from_path(filename: PathBuf) -> Self {
        Self {
            target: Target::File(filename),
        }
    }

    pub fn from_url(url: Url) -> Self {
        // create host from URL
        let host = Host::from_url(&url);
        Self {
            target: Target::Elasticsearch(ElasticsearchClient::new(host)),
        }
    }

    pub fn from_host(host: Host) -> Self {
        let app = match &host {
            Host::ApiKey { app, .. } | Host::Basic { app, .. } | Host::None { app, .. } => {
                app.clone()
            }
        };

        let target = match app {
            Product::Elasticsearch => Target::Elasticsearch(ElasticsearchClient::new(host)),
            _ => panic!("Output application can only be Elasticsearch"),
        };

        Self { target }
    }

    pub fn from_uri(uri: Uri) -> Self {
        match uri {
            Uri::Host(host) => Self::from_host(host),
            Uri::Url(url) => Self::from_url(url),
            Uri::File(filename) => Self::from_path(filename),
            Uri::Directory(dir) => panic!("Cannout output to a directory: {:?}", dir),
            Uri::Stream => Self::new(),
        }
    }

    pub async fn send(&self, docs: Vec<Value>) -> std::io::Result<usize> {
        match &self.target {
            Target::Stdout => {
                println!("{}", serde_json::to_string(&docs).unwrap());
                Ok(docs.len())
            }
            Target::File(filename) => file::append_bulk_docs(docs, &filename),
            Target::Elasticsearch(client) => client.bulk_index(docs).await,
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Target::Elasticsearch(_) => write!(f, "elasticsearch"),
            Target::File(filename) => write!(
                f,
                "{}",
                filename.to_str().expect("Failed to get filename as str")
            ),
            Target::Stdout => write!(f, "stdout"),
        }
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.target)
    }
}
