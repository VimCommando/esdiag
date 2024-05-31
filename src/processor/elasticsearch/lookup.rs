use super::EsDataSet::*;
use crate::input::DataSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "lookup")]
pub enum Lookup {
    #[serde(rename = "alias")]
    AliasLookup(AliasLookup),
    #[serde(rename = "data_stream")]
    DataStreamLookup(DataStreamLookup),
    #[serde(rename = "node")]
    NodeLookup(NodeLookup),
}

impl Lookup {
    pub fn new(data: DataSet, value: Value) -> Lookup {
        match data {
            DataSet::Elasticsearch(Alias) => Lookup::AliasLookup(build_alias_lookup(value)),
            DataSet::Elasticsearch(DataStreams) => {
                Lookup::DataStreamLookup(build_data_stream_lookup(value))
            }
            DataSet::Elasticsearch(Nodes) => Lookup::NodeLookup(build_node_lookup(value)),
            _ => panic!("ERROR: Invalid lookup source"),
        }
    }

    pub fn to_value(&self) -> Value {
        let json = match serde_json::to_string(&self) {
            Ok(json) => json,
            Err(e) => panic!("ERROR: Failed to convert lookup to JSON {}", e),
        };

        let value = match serde_json::from_str(&json) {
            Ok(value) => value,
            Err(e) => panic!("ERROR: Failed to convert lookup to Value {}", e),
        };
        value
    }

    pub fn by_id(&self, id: &str) -> Option<Value> {
        match self {
            Lookup::NodeLookup(lookup) => match lookup.by_id.get(id) {
                Some(index) => Some(serde_json::to_value(&lookup.nodes[*index]).unwrap()),
                None => None,
            },
            _ => None,
        }
    }

    pub fn by_index(&self, index: &str) -> Option<Value> {
        match self {
            Lookup::AliasLookup(lookup) => match lookup.by_index.get(index) {
                Some(alias) => Some(serde_json::to_value(alias).unwrap()),
                None => None,
            },
            Lookup::DataStreamLookup(lookup) => match lookup.by_index.get(index) {
                Some(data_stream) => Some(data_stream.clone()),
                None => None,
            },
            _ => None,
        }
    }
}

// Nodes Lookup

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NodeLookup {
    nodes: Vec<NodeData>,
    by_id: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
    by_host: HashMap<String, usize>,
    by_ip: HashMap<String, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NodeData {
    attributes: Value,
    host: String,
    id: String,
    ip: String,
    name: String,
    roles: Value,
    version: String,
}

pub fn build_node_lookup(nodes: Value) -> NodeLookup {
    let mut node_lookup: NodeLookup = NodeLookup {
        by_id: HashMap::new(),
        by_name: HashMap::new(),
        by_host: HashMap::new(),
        by_ip: HashMap::new(),
        nodes: Vec::new(),
    };

    for (id, data) in nodes["nodes"].as_object().unwrap() {
        let node = NodeData {
            attributes: data["attributes"].clone(),
            host: data["host"].as_str().unwrap().to_string(),
            id: id.clone(),
            ip: data["ip"].as_str().unwrap().to_string(),
            name: data["name"].as_str().unwrap().to_string(),
            roles: data["roles"].clone(),
            version: data["version"].as_str().unwrap().to_string(),
        };
        let index = node_lookup.nodes.len();
        node_lookup.by_id.insert(node.id.to_string(), index);
        node_lookup.by_name.insert(node.name.clone(), index);
        node_lookup.by_host.insert(node.host.clone(), index);
        node_lookup.by_ip.insert(node.ip.clone(), index);
        node_lookup.nodes.push(node);
    }
    node_lookup
}

// Alias Lookup

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AliasLookup {
    by_index: HashMap<String, Option<AliasData>>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AliasData {
    alias: Option<String>,
    is_hidden: Option<bool>,
    is_write_index: Option<bool>,
}

fn build_alias_lookup(aliases: Value) -> AliasLookup {
    let mut alias_lookup: AliasLookup = AliasLookup {
        by_index: HashMap::new(),
    };

    for (index, data) in aliases.as_object().cloned().expect("aliases not an object") {
        //println!("{:?}, {:?}", index, data);
        if let Some(aliases) = data["aliases"].as_object() {
            for (name, props) in aliases {
                let alias_data = Some(AliasData {
                    alias: Some(String::from(name)),
                    is_write_index: props["is_write_index"].as_bool(),
                    is_hidden: props["is_hidden"].as_bool(),
                });
                alias_lookup
                    .by_index
                    .insert(index.clone(), alias_data.clone());
            }
        }
    }
    alias_lookup
}

// DataSet Stream Lookup

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DataStreamLookup {
    by_index: HashMap<String, Value>,
}

fn build_data_stream_lookup(data_streams: Value) -> DataStreamLookup {
    let mut data_stream_lookup: DataStreamLookup = DataStreamLookup {
        by_index: HashMap::new(),
    };

    for data_stream in data_streams["data_streams"].as_array().unwrap().clone() {
        for (i, index) in data_stream["indices"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .enumerate()
            .clone()
        {
            let last_index: usize = data_stream["indices"].as_array().unwrap().len() - 1;
            let is_write_index: bool = i == last_index;
            let mut data_stream_obj = data_stream.as_object().unwrap().clone();
            data_stream_obj.insert(
                "is_write_index".to_string(),
                serde_json::Value::Bool(is_write_index),
            );
            data_stream_obj.remove("indices");
            data_stream_lookup.by_index.insert(
                index["index_name"].as_str().unwrap().into(),
                serde_json::to_value(data_stream_obj).unwrap(),
            );
        }
    }
    data_stream_lookup
}
