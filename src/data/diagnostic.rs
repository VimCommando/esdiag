/// Trait for receiving data from a source
pub mod data_source;
/// Elastic Cloud Kubernetes diagnostic bundle
pub mod eck;
/// Elasticsearch diagnostic bundle
pub mod elasticsearch;
/// Kibana diagnostic bundle
pub mod kibana;
/// Logstash diagnostic bundle
pub mod logstash;
/// Diagnostic bundle manifest file
pub mod manifest;

pub use eck::ElasticCloudKubernetes;
pub use elasticsearch::Elasticsearch;
pub use kibana::Kibana;
pub use logstash::Logstash;
pub use manifest::Manifest;

use elasticsearch::EsDataSet;
use serde::{Deserialize, Serialize};

pub trait DataFamilies {
    fn get_data_sets(&self) -> Vec<DataSet>;
    fn get_lookup_sets(&self) -> Vec<DataSet>;
    fn get_metadata_sets(&self) -> Vec<DataSet>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataSet {
    Elasticsearch(EsDataSet),
    //Kibana(KbDataSet),
    //Logstash(LsDataSet),
}

impl ToString for DataSet {
    fn to_string(&self) -> String {
        match self {
            DataSet::Elasticsearch(data_set) => data_set.to_string(),
            //DataSet::Kibana(data_set) => data_set.to_string(),
            //DataSet::Logstash(data_set) => data_set.to_string(),
        }
    }
}

// Product enum to hold the Elasticsearch, Kibana, or Logstash product

#[derive(Debug, PartialEq, Hash, Clone, Eq, Serialize, Deserialize)]
pub enum Product {
    Agent,
    ECE,
    ECK,
    Elasticsearch,
    Kibana,
    Logstash,
    Unknown,
}

impl std::fmt::Display for Product {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent => write!(fmt, "Agent"),
            Self::ECE => write!(fmt, "ECE"),
            Self::ECK => write!(fmt, "ECK"),
            Self::Elasticsearch => write!(fmt, "Elasticsearch"),
            Self::Kibana => write!(fmt, "Kibana"),
            Self::Logstash => write!(fmt, "Logstash"),
            Self::Unknown => write!(fmt, "Unknown"),
        }
    }
}

impl std::str::FromStr for Product {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "agent" => Ok(Self::Agent),
            "ece" => Ok(Self::ECE),
            "eck" => Ok(Self::ECK),
            "es" | "elasticsearch" => Ok(Self::Elasticsearch),
            "kb" | "kibana" => Ok(Self::Kibana),
            "ls" | "logstash" => Ok(Self::Logstash),
            _ => Err(()),
        }
    }
}

impl Default for Product {
    fn default() -> Self {
        Self::Unknown
    }
}
