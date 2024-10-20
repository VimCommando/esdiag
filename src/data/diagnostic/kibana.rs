use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DataSet {
    Nodes,
}

impl std::fmt::Display for DataSet {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSet::Nodes => write!(fmt, "nodes"),
        }
    }
}
