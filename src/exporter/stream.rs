use super::Export;
use color_eyre::eyre::Result;
use serde_json::Value;

pub struct StreamExporter {}

impl StreamExporter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Export for StreamExporter {
    async fn write(&self, _index: String, docs: Vec<Value>) -> Result<usize> {
        log::debug!("Writing {} docs to stdout", docs.len());
        let doc_count = docs.len();
        for doc in docs {
            serde_json::to_writer(std::io::stdout(), &doc)?;
            println!();
        }
        Ok(doc_count)
    }

    async fn is_connected(&self) -> bool {
        true
    }
}

impl std::fmt::Display for StreamExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stdout")
    }
}
