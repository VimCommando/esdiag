pub mod elasticsearch;
use elasticsearch::EsDataSet;
pub mod kibana;
pub mod logstash;
use crate::input::{manifest::Manifest, DataSet};
use elasticsearch::metadata::Metadata;
use serde_json::Value;
use std::collections::HashMap;

pub struct Processor {
    pub metadata: Metadata,
}

impl Processor {
    pub fn new(manifest: &Manifest, metadata: &HashMap<String, Value>) -> Self {
        Processor {
            metadata: Metadata::new(manifest, metadata),
        }
    }

    pub async fn enrich(&self, dataset: &DataSet, data: Value) -> Vec<Value> {
        let empty = Vec::<Value>::new();
        match dataset {
            DataSet::Elasticsearch(es_dataset) => match es_dataset {
                EsDataSet::Alias => empty,
                EsDataSet::ClusterSettings => {
                    elasticsearch::cluster_settings::enrich(&self.metadata, data).await
                }
                EsDataSet::DataStreams => empty,
                EsDataSet::IndexSettings => {
                    elasticsearch::index_settings::enrich(&self.metadata, data).await
                }
                EsDataSet::IndexStats => {
                    elasticsearch::index_stats::enrich(&self.metadata, data).await
                }
                EsDataSet::Nodes => elasticsearch::nodes::enrich(&self.metadata, data).await,
                EsDataSet::NodesStats => {
                    elasticsearch::nodes_stats::enrich(&self.metadata, data).await
                }
                EsDataSet::Tasks => elasticsearch::tasks::enrich(&self.metadata, data).await,
                EsDataSet::Version => empty,
            },
            DataSet::Kibana(kb_dataset) => match kb_dataset {
                _ => unimplemented!("Kibana"),
            },
            DataSet::Logstash(ls_dataset) => match ls_dataset {
                _ => unimplemented!("Logstash"),
            },
        }
    }
}

//
//    pub fn enrich(&self, dataset: &DataSet, data: Value) -> serde_json::Value {
//        match &source {
//            EsDataSet::Elasticsearch(_, source) => match source {
//                EsDataSet::ClusterSettings => return cluster_settings::enrich(&metadata, data),
//                elasticsearch::EsDataSet::IndexSettings => {
//                    return index_settings::enrich(&metadata, data)
//                }
//                elasticsearch::EsDataSet::IndicesStats => {
//                    return indices_stats::enrich(&metadata, data)
//                }
//                elasticsearch::EsDataSet::Nodes => return nodes::enrich(&metadata, data),
//                elasticsearch::EsDataSet::Tasks => return tasks::enrich(&metadata, data),
//            },
//            EsDataSet::Lookup(lookup) => serde_json::Value::Null,
//        }
//    }
