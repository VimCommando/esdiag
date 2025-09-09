// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::Export;
use crate::processor::{BatchResponse, DiagnosticReport, Identifiers, ProcessorSummary};
use eyre::Result;
use serde::Serialize;

#[derive(Clone)]
pub struct StreamExporter {
    pub identifiers: Identifiers,
}

impl StreamExporter {
    pub fn new() -> Self {
        Self {
            identifiers: Identifiers::default(),
        }
    }
}

impl Export for StreamExporter {
    fn with_identifiers(self, identifiers: Identifiers) -> Self {
        Self {
            identifiers,
            ..self
        }
    }

    async fn is_connected(&self) -> bool {
        true
    }

    async fn write<T>(&self, summary: &mut ProcessorSummary, docs: Vec<T>) -> Result<()>
    where
        T: Sized + Serialize,
    {
        let doc_count = docs.len() as u32;
        let start_time = std::time::Instant::now();
        let mut batch = BatchResponse::new(doc_count);
        log::debug!("Writing {} docs to stdout", doc_count);
        for doc in docs {
            serde_json::to_writer(std::io::stdout(), &doc)?;
            println!();
        }
        batch.size = doc_count;
        batch.time = start_time.elapsed().as_secs() as u32;
        batch.time = start_time.elapsed().as_millis() as u32;
        summary.add_batch(batch);
        Ok(())
    }

    async fn save_report(&self, report: &DiagnosticReport) -> Result<()> {
        crate::data::save_file("report.json", report)
    }
}

impl std::fmt::Display for StreamExporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stdout")
    }
}
