use super::{DiagnosticManifest, DiagnosticMetadata, Lookup, Product};
use color_eyre::eyre::Result;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize, Clone)]
pub struct DiagnosticReport {
    pub lookups: HashMap<String, LookupSummary>,
    pub processors: HashMap<String, ProcessorSummary>,
    pub product: Product,
    pub docs_total: u32,
    #[serde(flatten)]
    pub metadata: DiagnosticMetadata,
}

impl DiagnosticReport {
    pub fn try_new(product: Product, manifest: DiagnosticManifest) -> Result<Self> {
        let metadata = DiagnosticMetadata::try_from(manifest)?;
        Ok(Self {
            docs_total: 0,
            lookups: HashMap::new(),
            metadata,
            processors: HashMap::new(),
            product,
        })
    }

    pub fn with_product(self, product: Product) -> Self {
        Self { product, ..self }
    }

    pub fn add_processor_summary(&mut self, summary: ProcessorSummary) {
        self.docs_total += summary.docs;
        self.processors.insert(summary.processor.clone(), summary);
    }

    pub fn add_lookup<T>(&mut self, name: &str, lookup: &Lookup<T>)
    where
        T: Clone + Serialize,
    {
        let summary = LookupSummary {
            count: lookup.len() as u32,
        };
        self.lookups.insert(name.to_string(), summary);
    }
}

#[derive(Serialize, Clone)]
pub struct BatchResponse {
    pub docs: u32,
    errors: u32,
    retries: u16,
    size: u32,
    status_code: u16,
    time: u32,
}

impl BatchResponse {
    pub fn new(docs: u32) -> Self {
        Self {
            docs,
            errors: 0,
            retries: 0,
            size: 0,
            status_code: 0,
            time: 0,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ProcessorSummary {
    avg_size: u32,
    avg_time: u32,
    batch_count: u32,
    batch_errors: u32,
    batch_retries: u16,
    batch_status_codes: HashMap<u16, u32>,
    pub docs: u32,
    doc_errors: u32,
    #[serde(skip_serializing)]
    pub processor: String,
    pub source_parsed: bool,
    #[serde(skip_serializing)]
    pub batches: Vec<BatchResponse>,
}

impl ProcessorSummary {
    pub fn new(name: String) -> Self {
        Self {
            avg_size: 0,
            avg_time: 0,
            batch_count: 0,
            batch_errors: 0,
            batch_retries: 0,
            batch_status_codes: HashMap::new(),
            docs: 0,
            doc_errors: 0,
            processor: name,
            source_parsed: false,
            batches: Vec::new(),
        }
    }

    pub fn add_batch(&mut self, batch: BatchResponse) {
        self.batch_count += 1;
        self.batch_errors += batch.errors;
        self.batch_retries += batch.retries;
        self.batch_status_codes
            .entry(batch.status_code)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.docs += batch.docs;
        self.batches.push(batch);
    }
}

#[derive(Serialize, Clone)]
pub struct LookupSummary {
    count: u32,
}

impl From<DiagnosticMetadata> for DiagnosticReport {
    fn from(metadata: DiagnosticMetadata) -> Self {
        Self {
            docs_total: 0,
            lookups: HashMap::new(),
            metadata,
            processors: HashMap::new(),
            product: Product::Unknown,
        }
    }
}
