use serde::{Deserialize, Serialize};

use crate::data::elasticsearch::{
    AliasList, ClusterSettings, DataStreams, IlmExplain, IndicesSettings, IndicesStats, Nodes,
    NodesStats, SearchableSnapshotsCacheStats, SearchableSnapshotsStats, Tasks, Version,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
pub enum DataSet {
    Aliases,
    ClusterSettings,
    DataStreams,
    IlmExplain,
    IndicesSettings,
    IndicesStats,
    Nodes,
    NodesStats,
    SearchableSnapshotsCacheStats,
    SearchableSnapshotsStats,
    Tasks,
    Version,
}

impl std::fmt::Display for DataSet {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSet::Aliases => write!(fmt, "aliases"),
            DataSet::ClusterSettings => write!(fmt, "cluster_settings"),
            DataSet::DataStreams => write!(fmt, "data_streams"),
            DataSet::IlmExplain => write!(fmt, "ilm_explain"),
            DataSet::IndicesSettings => write!(fmt, "indices_settings"),
            DataSet::IndicesStats => write!(fmt, "indices_stats"),
            DataSet::Nodes => write!(fmt, "nodes"),
            DataSet::NodesStats => write!(fmt, "nodes_stats"),
            DataSet::SearchableSnapshotsCacheStats => {
                write!(fmt, "searchable_snapshots_cache_stats")
            }
            DataSet::SearchableSnapshotsStats => write!(fmt, "searchable_snapshots_stats"),
            DataSet::Tasks => write!(fmt, "tasks"),
            DataSet::Version => write!(fmt, "version"),
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

impl From<AliasList> for DataSet {
    fn from(_: AliasList) -> Self {
        DataSet::Aliases
    }
}

impl From<ClusterSettings> for DataSet {
    fn from(_: ClusterSettings) -> Self {
        DataSet::ClusterSettings
    }
}

impl From<DataStreams> for DataSet {
    fn from(_: DataStreams) -> Self {
        DataSet::DataStreams
    }
}

impl From<IlmExplain> for DataSet {
    fn from(_: IlmExplain) -> Self {
        DataSet::IlmExplain
    }
}

impl From<IndicesSettings> for DataSet {
    fn from(_: IndicesSettings) -> Self {
        DataSet::IndicesSettings
    }
}

impl From<IndicesStats> for DataSet {
    fn from(_: IndicesStats) -> Self {
        DataSet::IndicesStats
    }
}

impl From<Nodes> for DataSet {
    fn from(_: Nodes) -> Self {
        DataSet::Nodes
    }
}

impl From<NodesStats> for DataSet {
    fn from(_: NodesStats) -> Self {
        DataSet::NodesStats
    }
}

impl From<SearchableSnapshotsCacheStats> for DataSet {
    fn from(_: SearchableSnapshotsCacheStats) -> Self {
        DataSet::SearchableSnapshotsCacheStats
    }
}

impl From<SearchableSnapshotsStats> for DataSet {
    fn from(_: SearchableSnapshotsStats) -> Self {
        DataSet::SearchableSnapshotsStats
    }
}

impl From<Tasks> for DataSet {
    fn from(_: Tasks) -> Self {
        DataSet::Tasks
    }
}

impl From<Version> for DataSet {
    fn from(_: Version) -> Self {
        DataSet::Version
    }
}
