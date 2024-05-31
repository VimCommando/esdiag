#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LsDataSet {
    Nodes,
}

impl ToString for LsDataSet {
    fn to_string(&self) -> String {
        match self {
            LsDataSet::Nodes => "nodes".to_string(),
        }
    }
}
