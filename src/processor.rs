/// Processors for diagnostic bundles
pub mod diagnostic;
/// Processors for Elasticsearch diagnostics
pub mod elasticsearch;
/// Lookup processors
mod lookup;

use diagnostic::DiagnosticProcessor;
use elasticsearch::ElasticsearchDiagnostic;

use crate::{
    data::diagnostic::{Manifest, Product},
    exporter::Exporter,
    receiver::Receiver,
};
use color_eyre::eyre::{eyre, Result};

pub async fn process(receiver: Receiver, exporter: Exporter) -> Result<()> {
    let manifest = receiver.get::<Manifest>().await?;
    let diagnostic_processor: Box<_> = match manifest.product {
        Product::Elasticsearch => {
            ElasticsearchDiagnostic::new(manifest, receiver, exporter).await?
        }
        _ => return Err(eyre!("Unsupported product or diagnostic bundle")),
    };

    diagnostic_processor.run().await?;
    Ok(())
}

pub trait DataProcessor {
    #[allow(async_fn_in_trait)]
    async fn process(&self) -> (String, Vec<serde_json::Value>);
}

pub trait Metadata {
    fn as_meta_doc(&self) -> serde_json::Value;
}
