use super::lookup::LookupTable;
use crate::{data::diagnostic::Manifest, exporter::Exporter, receiver::Receiver};
use color_eyre::Result;
use std::boxed::Box;
use std::sync::Arc;

pub trait DiagnosticProcessor {
    #[allow(async_fn_in_trait)]
    async fn new(manifest: Manifest, receiver: Receiver, exporter: Exporter) -> Result<Box<Self>>;
    #[allow(async_fn_in_trait)]
    fn get_lookup(&self, key: &str) -> Option<Arc<dyn LookupTable>>;
    #[allow(async_fn_in_trait)]
    async fn process_queue(&self) -> usize;
    #[allow(async_fn_in_trait)]
    async fn run(self) -> Result<(String, usize)>;
}
