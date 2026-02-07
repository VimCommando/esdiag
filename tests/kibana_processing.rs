// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use esdiag::data::Uri;
use esdiag::exporter::Exporter;
use esdiag::processor::{Identifiers, Processor};
use esdiag::receiver::Receiver;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn test_kibana_processing() {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/archives/kibana-api-diagnostics-8.19.3.zip");
    let input = d.to_str().unwrap().to_string();
    let output = "-".to_string(); // stdout for testing

    let input_uri = Uri::try_from(input).expect("Failed to parse input URI");
    let output_uri = Uri::try_from(output).expect("Failed to parse output URI");

    let receiver = Arc::new(Receiver::try_from(input_uri).expect("Failed to create receiver"));
    let exporter = Arc::new(Exporter::try_from(output_uri).expect("Failed to create exporter"));

    let identifiers = Identifiers::new(None, None, receiver.filename(), None, None);
    let processor = Processor::try_new(receiver, exporter, identifiers)
        .await
        .expect("Failed to create processor");

    let processor = match processor.start().await {
        Ok(p) => p,
        Err(e) => panic!("Failed to start processor: {}", e),
    };
    let result = processor.process().await;

    if let Err(e) = result {
        panic!("Kibana processing failed: {}", e);
    }
}
