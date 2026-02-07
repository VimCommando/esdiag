// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

use super::super::super::diagnostic::DataSource;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Logs(pub String);

impl DataSource for Logs {
    fn name() -> String {
        "logs".to_string()
    }
}
