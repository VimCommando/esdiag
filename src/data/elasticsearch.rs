/// For the `_alias` API
mod aliases;
/// For the `_cluster/settings` API
mod cluster_settings;
/// For the `_data_streams` API
mod data_streams;
/// For the `_ilm/explain` API
mod ilm_explain;
/// For the `_settings` API
mod indices_settings;
/// For the `_stats` API
mod indices_stats;
/// For the `_nodes` API
mod nodes;
/// For the `_nodes/stats` API
mod nodes_stats;
/// For the `_searchable_snapshots/cache/stats` API
mod searchable_snapshots_cache_stats;
/// For the `_searchable_snapshots/stats` API
mod searchable_snapshots_stats;
/// For the `_tasks` API
mod tasks;
/// For the root `/` API
mod version;

pub use aliases::*;
pub use cluster_settings::*;
pub use data_streams::*;
pub use ilm_explain::*;
pub use indices_settings::*;
pub use indices_stats::*;
pub use nodes::*;
pub use nodes_stats::*;
pub use searchable_snapshots_cache_stats::*;
pub use searchable_snapshots_stats::*;
pub use tasks::*;
pub use version::*;
