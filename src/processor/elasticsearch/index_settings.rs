use super::metadata::Metadata;
use json_patch::merge;
use rayon::prelude::*;
use serde_json::{json, Value};

pub async fn enrich(metadata: &Metadata, data: Value) -> Vec<Value> {
    let indices: Vec<_> = match data.as_object() {
        Some(data) => data.into_iter().collect(),
        None => return Vec::<Value>::new(),
    };
    log::debug!("indices: {}", indices.len());

    let data_stream = json!({
        "data_stream": {
            "dataset": "index",
            "namespace": "esdiag",
            "type": "settings",
        }
    });

    let index_settings: Vec<Value> = indices
        .par_iter()
        .map(|(index, settings)| {
            let mut doc = json!({
                "@timestamp": metadata.diagnostic.collection_date,
                "cluster": metadata.cluster,
                "data_stream": metadata.data_stream_lookup.by_index(index.as_str()),
                "diagnostic": metadata.diagnostic,
                "index": settings["settings"]["index"],
            });
            let doc_patch = json!({
                "index": {
                    "name": index
                },
            });
            merge(&mut doc, &doc_patch);
            merge(&mut doc, &data_stream);
            doc
        })
        .collect();

    log::debug!("index setting docs: {}", index_settings.len());
    index_settings
}
