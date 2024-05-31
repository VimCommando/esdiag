use super::{Application, DataSet};
use crate::processor::elasticsearch::EsDataSet::*;

pub struct Elasticsearch {
    pub metadata_sets: Vec<DataSet>,
    pub data_sets: Vec<DataSet>,
}

impl Elasticsearch {
    pub fn new() -> Box<dyn Application> {
        let metadata_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(Alias),
            DataSet::Elasticsearch(DataStreams),
            DataSet::Elasticsearch(Nodes),
            DataSet::Elasticsearch(Version),
        ]);
        let data_sets: Vec<DataSet> = Vec::from([
            DataSet::Elasticsearch(ClusterSettings),
            DataSet::Elasticsearch(IndexSettings),
            DataSet::Elasticsearch(IndexStats),
            DataSet::Elasticsearch(Nodes),
            DataSet::Elasticsearch(NodesStats),
            DataSet::Elasticsearch(Tasks),
        ]);

        Box::new(Self {
            metadata_sets,
            data_sets,
        })
    }
}

impl Application for Elasticsearch {
    fn get_metadata_sets(&self) -> Vec<DataSet> {
        self.metadata_sets.clone()
    }

    fn get_data_sets(&self) -> Vec<DataSet> {
        self.data_sets.clone()
    }
}
