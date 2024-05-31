use super::{Application, DataSet};

pub struct Logstash {
    metadata_sets: Vec<DataSet>,
    data_sets: Vec<DataSet>,
}

impl Logstash {
    pub fn new() -> Box<dyn Application> {
        Box::new(Self {
            metadata_sets: Vec::new(),
            data_sets: Vec::new(),
        })
    }
}

impl Application for Logstash {
    fn get_metadata_sets(&self) -> Vec<DataSet> {
        self.metadata_sets.clone()
    }

    fn get_data_sets(&self) -> Vec<DataSet> {
        self.data_sets.clone()
    }
}
