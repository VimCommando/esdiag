use super::Lookup;
use crate::data::elasticsearch::{DataStream, DataStreams, Indices};
use serde::{Deserialize, Serialize};

impl From<&String> for Lookup<DataStream> {
    fn from(string: &String) -> Self {
        let data_streams: DataStreams =
            serde_json::from_str(&string).expect("Failed to parse DataStreamData");
        Lookup::<DataStream>::from(data_streams)
    }
}

impl From<DataStreams> for Lookup<DataStream> {
    fn from(mut data_streams: DataStreams) -> Self {
        let mut lookup = Lookup::<DataStream>::new();
        data_streams
            .data_streams
            .drain(..)
            .enumerate()
            .for_each(|(i, mut data_stream)| {
                let name = data_stream.name.clone();
                let indices: Indices = data_stream.indices.drain(..).collect();
                let index_count = indices.len() - 1;
                data_stream.set_write_index(i == index_count);
                lookup.add(data_stream).with_name(&name);
                // Each data stream can have multiple indices
                indices.iter().for_each(|index| {
                    lookup.with_id(&index.index_name.clone());
                });
            });

        log::debug!("lookup data_stream entries: {}", lookup.len(),);
        lookup
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TimestampField {
    name: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Index {
    index_name: String,
    index_uuid: String,
    prefer_ilm: Option<bool>,
    ilm_policy: Option<String>,
    managed_by: Option<String>,
}
