use crate::data::diagnostic::{DataSource, data_source::PathType, elasticsearch::DataSet};
use eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct IndicesStats {
    _shards: Value,
    _all: Value,
    pub indices: HashMap<String, IndexStats>,
}

#[derive(Deserialize)]
pub struct IndexStats {
    pub uuid: Option<String>,
    pub health: Option<String>,
    pub primaries: Stats,
    pub total: Stats,
    #[serde(skip_serializing)]
    pub shards: Option<HashMap<String, Value>>,
}

#[derive(Deserialize, Serialize)]
pub struct Stats {
    pub bulk: Option<Bulk>,
    pub completion: Option<Completion>,
    pub dense_vector: Option<DenseVector>,
    pub docs: Option<Docs>,
    pub fielddata: Option<Fielddata>,
    pub flush: Option<Flush>,
    pub get: Option<Get>,
    pub indexing: Option<Indexing>,
    pub merges: Option<Merges>,
    pub query_cache: Option<QueryCache>,
    pub recovery: Option<Recovery>,
    pub refresh: Option<Refresh>,
    pub request_cache: Option<RequestCache>,
    pub search: Option<Search>,
    pub segments: Option<Segments>,
    pub shard_stats: ShardStats,
    pub sparse_vector: Option<SparseVector>,
    pub store: Option<StoreStats>,
    pub translog: Option<Translog>,
    pub warmer: Option<Warmer>,
}

#[derive(Deserialize, Serialize)]
pub struct Docs {
    pub count: Option<u64>,
    pub deleted: Option<u64>,
    pub total_size_in_bytes: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct ShardStats {
    pub total_count: u64,
}

#[derive(Deserialize, Serialize)]
pub struct StoreStats {
    pub size_in_bytes: u64,
    pub total_data_set_size_in_bytes: Option<u64>,
    pub reserved_in_bytes: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Indexing {
    pub index_total: Option<u64>,
    pub index_time_in_millis: Option<u64>,
    pub index_current: Option<u64>,
    pub index_failed: Option<u64>,
    pub delete_total: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Get {
    pub total: Option<u64>,
    pub time_in_millis: Option<u64>,
    pub exists_total: Option<u64>,
    pub exists_time_in_millis: Option<u64>,
    pub missing_total: Option<u64>,
    pub missing_time_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Search {
    pub open_contexts: Option<u64>,
    pub query_total: Option<u64>,
    pub query_time_in_millis: Option<u64>,
    pub query_current: Option<u64>,
    pub query_failure: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Merges {
    pub current: Option<u64>,
    pub current_docs: Option<u64>,
    pub current_size_in_bytes: Option<u64>,
    pub total: Option<u64>,
    pub total_time_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Refresh {
    pub total: Option<u64>,
    pub total_time_in_millis: Option<u64>,
    pub external_total: Option<u64>,
    pub external_total_time_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Flush {
    pub total: Option<u64>,
    pub periodic: Option<u64>,
    pub total_time_in_millis: Option<u64>,
    pub total_time_excluding_waiting_on_lock_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Warmer {
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub total_time_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct QueryCache {
    pub memory_size_in_bytes: Option<u64>,
    pub total_count: Option<u64>,
    pub hit_count: Option<u64>,
    pub miss_count: Option<u64>,
    pub cache_size: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Fielddata {
    pub memory_size_in_bytes: Option<u64>,
    pub evictions: Option<u64>,
    pub global_ordinals: Option<Value>,
}

#[derive(Deserialize, Serialize)]
pub struct Completion {
    pub size_in_bytes: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Segments {
    pub count: Option<u64>,
    pub memory_in_bytes: Option<u64>,
    pub terms_memory_in_bytes: Option<u64>,
    pub stored_fields_memory_in_bytes: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Translog {
    pub operations: Option<u64>,
    pub size_in_bytes: Option<u64>,
    pub uncommitted_operations: Option<u64>,
    pub uncommitted_size_in_bytes: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct RequestCache {
    pub memory_size_in_bytes: Option<u64>,
    pub evictions: Option<u64>,
    pub hit_count: Option<u64>,
    pub miss_count: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Recovery {
    pub current_as_source: Option<u64>,
    pub current_as_target: Option<u64>,
    pub throttle_time_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct Bulk {
    pub total_operations: Option<u64>,
    pub total_time_in_millis: Option<u64>,
    pub total_size_in_bytes: Option<u64>,
    pub avg_time_in_millis: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct DenseVector {
    pub value_count: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct SparseVector {
    pub value_count: Option<u64>,
}

impl DataSource for IndicesStats {
    fn source(path: PathType) -> Result<&'static str> {
        match path {
            PathType::File => Ok("indices_stats.json"),
            PathType::Url => Ok("_all/_stats?level=shards"),
        }
    }

    fn name() -> String {
        format!("{}", DataSet::IndicesStats)
    }
}
