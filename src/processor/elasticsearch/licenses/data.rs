use super::super::super::diagnostic::data_source::PathType;
use super::super::DataSource;
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Deserialize)]
pub struct Licenses {
    pub license: License,
}

#[skip_serializing_none]
#[derive(Clone, Deserialize, Serialize)]
pub struct License {
    status: String,
    uid: String,
    r#type: String,
    //issue_date: String,
    issue_date_in_millis: i64,
    //expiry_date: String,
    expiry_date_in_millis: i64,
    max_nodes: Option<i32>,
    max_resource_units: i32,
    issued_to: String,
    issuer: String,
    start_date_in_millis: i64,
}

impl DataSource for Licenses {
    fn source(kind: PathType) -> Result<&'static str> {
        match kind {
            PathType::File => Ok("licenses.json"),
            PathType::Url => Ok("_license"),
        }
    }

    fn name() -> String {
        "licenses".to_string()
    }
}
