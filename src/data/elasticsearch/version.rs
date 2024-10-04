use crate::data::{diagnostic::data_source::DataSource, Uri};
use color_eyre::eyre::{eyre, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Cluster {
    #[serde(rename = "name")]
    pub node_name: String,
    #[serde(rename = "cluster_name")]
    pub name: String,
    #[serde(rename = "cluster_uuid")]
    pub uuid: String,
    pub version: Version,
    //pub tagline: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Version {
    pub number: semver::Version,
    pub build_flavor: String,
    pub build_type: String,
    pub build_hash: String,
    pub build_date: String,
    pub build_snapshot: bool,
    pub lucene_version: String,
    pub minimum_wire_compatibility_version: String,
    pub minimum_index_compatibility_version: String,
}

impl DataSource for Cluster {
    fn source(uri: &Uri) -> Result<&'static str> {
        match uri {
            Uri::Directory(_) => Ok("version.json"),
            Uri::Host(_) | Uri::Url(_) => Ok("/"),
            _ => Err(eyre!("Unsupported source for version")),
        }
    }
}
