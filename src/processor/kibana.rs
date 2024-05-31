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
