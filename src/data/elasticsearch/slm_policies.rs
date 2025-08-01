use crate::data::diagnostic::{DataSource, data_source::PathType, elasticsearch::DataSet};
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::skip_serializing_none;
use std::collections::HashMap;

pub type SlmPolicies = HashMap<String, SlmPolicy>;

#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
pub struct SlmPolicy {
    version: u32,
    // modified_date: String,
    modified_date_millis: u64,
    policy: Value,
    last_success: Option<Value>,
    last_failure: Option<Value>,
    // next_execution: Option<String>,
    next_execution_millis: Option<u64>,
    stats: Option<Value>,
}

impl DataSource for SlmPolicies {
    fn source(path: PathType) -> Result<&'static str> {
        match path {
            PathType::File => Ok("commercial/slm_policies.json"),
            PathType::Url => Ok("/_slm/policy"),
        }
    }

    fn name() -> String {
        format!("{}", DataSet::IlmPolicies)
    }
}
