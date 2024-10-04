use super::{DataProcessor, ElasticsearchDiagnostic, Receiver};
use crate::{
    data::elasticsearch::{Node, Nodes},
    processor::Metadata,
};
use json_patch::merge;
use rayon::prelude::*;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct NodesProcessor {
    diagnostic: Arc<ElasticsearchDiagnostic>,
    receiver: Arc<Receiver>,
}

impl NodesProcessor {
    fn new(diagnostic: Arc<ElasticsearchDiagnostic>, receiver: Arc<Receiver>) -> Self {
        NodesProcessor {
            diagnostic,
            receiver,
        }
    }
}

impl From<Arc<ElasticsearchDiagnostic>> for NodesProcessor {
    fn from(diagnostic: Arc<ElasticsearchDiagnostic>) -> Self {
        NodesProcessor::new(diagnostic.clone(), diagnostic.receiver.clone())
    }
}
impl DataProcessor for NodesProcessor {
    async fn process(&self) -> (String, Vec<Value>) {
        let data_stream = "settings-nodes-esdiag".to_string();
        let node_lookup = &self.diagnostic.lookups.node;
        let metadata = self
            .diagnostic
            .metadata
            .for_data_stream(&data_stream)
            .as_meta_doc();
        let mut nodes = match self.receiver.get::<Nodes>().await {
            Ok(nodes) => nodes.nodes,
            Err(e) => {
                log::warn!("Failed to deserialize nodes: {}", e);
                return (data_stream, Vec::new());
            }
        };

        log::debug!("nodes: {}", nodes.len());

        let node_doc = NodeDoc {
            metadata,
            node: None,
        };

        let node_docs: Vec<Value> = nodes
            .par_drain()
            .map(|(node_id, node)| {
                let node_summary = node_lookup.by_id(&node_id).cloned();
                let patch = json!({
                    "node" : {
                        "settings": {
                            "http": {
                                "type.default": null,
                            },
                            "transport": {
                                "type.default": null,
                            },
                        }
                    }
                });

                let mut node_doc = json!(node_doc.clone().with_node(node));
                merge(&mut node_doc, &patch);
                if let Ok(node_summary) = serde_json::to_value(node_summary) {
                    merge(&mut node_doc, &node_summary)
                }
                node_doc
            })
            .collect();

        log::debug!("node settings docs: {}", node_docs.len());
        (data_stream, node_docs)
    }
}

#[derive(Clone, Serialize)]
struct NodeDoc {
    #[serde(flatten)]
    metadata: Value,
    node: Option<Node>,
}

impl NodeDoc {
    fn with_node(self, node: Node) -> Self {
        Self {
            node: Some(node),
            ..self
        }
    }
}
