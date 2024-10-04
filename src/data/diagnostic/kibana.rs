use super::{DataFamilies, DataSet};

pub struct Kibana {
    data_sets: Vec<DataSet>,
    lookup_sets: Vec<DataSet>,
    metadata_sets: Vec<DataSet>,
}

impl Kibana {
    pub fn new() -> Box<dyn DataFamilies> {
        Box::new(Self {
            data_sets: Vec::new(),
            lookup_sets: Vec::new(),
            metadata_sets: Vec::new(),
        })
    }
}

impl DataFamilies for Kibana {
    fn get_data_sets(&self) -> Vec<DataSet> {
        self.data_sets.clone()
    }

    fn get_lookup_sets(&self) -> Vec<DataSet> {
        self.lookup_sets.clone()
    }

    fn get_metadata_sets(&self) -> Vec<DataSet> {
        self.metadata_sets.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KbDataSet {
    Nodes,
}

impl ToString for KbDataSet {
    fn to_string(&self) -> String {
        match self {
            KbDataSet::Nodes => "nodes".to_string(),
        }
    }
}
