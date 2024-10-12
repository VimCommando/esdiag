use super::{DataFamilies, DataSet};
use serde::{Deserialize, Serialize};

/// Defines the known data sets from an Elasticsearch diagnostic

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ElasticsearchDataSet {
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

use ElasticsearchDataSet::*;

pub struct Elasticsearch {
    pub data_sets: Vec<DataSet>,
    pub lookup_sets: Vec<DataSet>,
    pub metadata_sets: Vec<DataSet>,
}

impl Elasticsearch {
    pub fn new() -> Box<dyn DataFamilies> {
        let metadata_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(Alias),
            DataSet::Elasticsearch(Version),
            DataSet::Elasticsearch(DataStreams),
            DataSet::Elasticsearch(IlmExplain),
            DataSet::Elasticsearch(SharedCacheStats),
        ]);
        let lookup_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(Nodes),
            DataSet::Elasticsearch(IndexSettings),
            DataSet::Elasticsearch(SearchableSnapshotStats),
        ]);
        let data_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(ClusterSettings),
            DataSet::Elasticsearch(Tasks),
            DataSet::Elasticsearch(IndexStats),
            DataSet::Elasticsearch(NodesStats),
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

impl ToString for ElasticsearchDataSet {
    fn to_string(&self) -> String {
        match self {
            Alias => "alias".to_string(),
            DataStreams => "data_stream".to_string(),
            Nodes => "nodes".to_string(),
            Version => "version".to_string(),
            ClusterSettings => "cluster_settings_defaults".to_string(),
            IlmExplain => "ilm_explain".to_string(),
            IndexSettings => "settings".to_string(),
            IndexStats => "indices_stats".to_string(),
            NodesStats => "nodes_stats".to_string(),
            SharedCacheStats => "searchable_snapshots_cache_stats".to_string(),
            SearchableSnapshotStats => "searchable_snapshots_stats".to_string(),
            Tasks => "tasks".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ElasticsearchVersionDetails {
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
pub struct ElasticsearchVersion {
    pub name: String,
    pub cluster_name: String,
    pub cluster_uuid: String,
    pub version: ElasticsearchVersionDetails,
    pub tagline: String,
}
