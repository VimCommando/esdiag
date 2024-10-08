/// Export to an Elasticsearch cluster with the `_bulk` API
mod elasticsearch;
/// Write to an `.ndjson` file
mod file;
/// Write `ndjson` to std out
mod stream;

use crate::data::Uri;
use color_eyre::eyre::{eyre, Result};
use elasticsearch::ElasticsearchExporter;
use file::FileExporter;
use serde_json::Value;
use stream::StreamExporter;

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
