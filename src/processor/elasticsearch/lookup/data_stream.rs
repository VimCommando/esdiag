use crate::{
    data::elasticsearch::{DataStream, DataStreams, Indices},
    processor::lookup::Lookup,
};
use color_eyre::eyre::Result;

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

impl From<Result<DataStreams>> for Lookup<DataStream> {
    fn from(data_streams: Result<DataStreams>) -> Self {
        match data_streams {
            Ok(data_streams) => Lookup::<DataStream>::from(data_streams),
            Err(e) => {
                log::warn!("Failed to parse DataStreams: {}", e);
                Lookup::new()
            }
        }
    }
}
