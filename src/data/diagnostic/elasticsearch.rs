use super::{DataFamilies, DataSet};
use serde::{Deserialize, Serialize};

/// Defines the data sets from an Elasticsearch diagnostic

pub struct Elasticsearch {
    pub data_sets: Vec<DataSet>,
    pub lookup_sets: Vec<DataSet>,
    pub metadata_sets: Vec<DataSet>,
}

impl Elasticsearch {
    pub fn new() -> Box<dyn DataFamilies> {
        let metadata_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(EsDataSet::Alias),
            DataSet::Elasticsearch(EsDataSet::Version),
            DataSet::Elasticsearch(EsDataSet::DataStreams),
            DataSet::Elasticsearch(EsDataSet::IlmExplain),
            DataSet::Elasticsearch(EsDataSet::SharedCacheStats),
        ]);
        let lookup_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(EsDataSet::Nodes),
            DataSet::Elasticsearch(EsDataSet::IndexSettings),
            DataSet::Elasticsearch(EsDataSet::SearchableSnapshotStats),
        ]);
        let data_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(EsDataSet::ClusterSettings),
            DataSet::Elasticsearch(EsDataSet::Tasks),
            DataSet::Elasticsearch(EsDataSet::IndexStats),
            DataSet::Elasticsearch(EsDataSet::NodesStats),
        ]);

        Box::new(Self {
            data_sets,
            lookup_sets,
            metadata_sets,
        })
    }
}

impl DataFamilies for Elasticsearch {
    fn get_metadata_sets(&self) -> Vec<DataSet> {
        self.metadata_sets.clone()
    }

    fn get_lookup_sets(&self) -> Vec<DataSet> {
        self.lookup_sets.clone()
    }

    fn get_data_sets(&self) -> Vec<DataSet> {
        self.data_sets.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EsDataSet {
    Alias,
    DataStreams,
    Nodes,
    Version,
    ClusterSettings,
    IlmExplain,
    IndexSettings,
    IndexStats,
    NodesStats,
    SharedCacheStats,
    SearchableSnapshotStats,
    Tasks,
}

impl ToString for EsDataSet {
    fn to_string(&self) -> String {
        match self {
            EsDataSet::Alias => "alias".to_string(),
            EsDataSet::DataStreams => "data_stream".to_string(),
            EsDataSet::Nodes => "nodes".to_string(),
            EsDataSet::Version => "version".to_string(),
            EsDataSet::ClusterSettings => "cluster_settings_defaults".to_string(),
            EsDataSet::IlmExplain => "ilm_explain".to_string(),
            EsDataSet::IndexSettings => "settings".to_string(),
            EsDataSet::IndexStats => "indices_stats".to_string(),
            EsDataSet::NodesStats => "nodes_stats".to_string(),
            EsDataSet::SharedCacheStats => "searchable_snapshots_cache_stats".to_string(),
            EsDataSet::SearchableSnapshotStats => "searchable_snapshots_stats".to_string(),
            EsDataSet::Tasks => "tasks".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EsVersionDetails {
    pub number: semver::Version,
    pub build_flavor: String,
    pub build_type: String,
    pub build_hash: String,
    pub build_date: String,
    pub build_snapshot: bool,
    pub lucene_version: String,
    pub minimum_wire_compatibility_version: semver::Version,
    pub minimum_index_compatibility_version: semver::Version,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EsVersion {
    pub name: String,
    pub cluster_name: String,
    pub cluster_uuid: String,
    pub version: EsVersionDetails,
    pub tagline: String,
}
