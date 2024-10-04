use super::{DataProcessor, ElasticsearchDiagnostic, Receiver};
use crate::{
    data::elasticsearch::{ParentTask, Task, Tasks},
    processor::{lookup::elasticsearch::node::NodeSummary, Metadata},
};
use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

pub struct TasksProcessor {
    diagnostic: Arc<ElasticsearchDiagnostic>,
    receiver: Arc<Receiver>,
}

impl TasksProcessor {
    fn new(diagnostic: Arc<ElasticsearchDiagnostic>, receiver: Arc<Receiver>) -> Self {
        TasksProcessor {
            diagnostic,
            receiver,
        }
    }
}

impl From<Arc<ElasticsearchDiagnostic>> for TasksProcessor {
    fn from(diagnostic: Arc<ElasticsearchDiagnostic>) -> Self {
        TasksProcessor::new(diagnostic.clone(), diagnostic.receiver.clone())
    }
}

impl DataProcessor for TasksProcessor {
    async fn process(&self) -> (String, Vec<Value>) {
        let data_stream = "metrics-task-esdiag".to_string();
        let tasks = match self.receiver.get::<Tasks>().await {
            Ok(tasks) => tasks,
            Err(e) => {
                log::error!("Failed to deserialize tasks: {}", e);
                return (data_stream, Vec::new());
            }
        };
        let lookup_node = &self.diagnostic.lookups.node;
        let task_metadata = self
            .diagnostic
            .metadata
            .for_data_stream(&data_stream)
            .as_meta_doc();

        let nodes: Vec<(_, _)> = tasks.nodes.into_par_iter().collect();

        let tasks: Vec<Value> = nodes
            .into_par_iter()
            .flat_map(|(node_id, node)| {
                node.tasks
                    .iter()
                    .collect::<Vec<_>>()
                    .into_par_iter()
                    .map(|(_, task)| {
                        let node = lookup_node
                            .by_id(node_id.as_str())
                            .cloned()
                            .expect("Node not found for task");
                        serde_json::to_value(TaskDoc::new(task, task_metadata.clone(), node))
                            .unwrap_or_default()
                    })
                    .collect::<Vec<Value>>()
            })
            .collect();

        log::debug!("task docs: {}", tasks.len());
        (data_stream, tasks)
    }
}

#[derive(Clone, Serialize)]
pub struct TaskDoc {
    #[serde(flatten)]
    metadata: Value,
    node: NodeSummary,
    task: TaskWithParent,
}

impl TaskDoc {
    pub fn new(task: &Task, metadata: Value, node: NodeSummary) -> Self {
        let parent = task
            .parent_task_id
            .as_ref()
            .map(|id| ParentTask::from(id.clone()));
        TaskDoc {
            metadata: metadata.clone(),
            node,
            task: TaskWithParent {
                task: task.clone(),
                parent,
            },
        }
    }
}

#[derive(Clone, Serialize)]
pub struct TaskWithParent {
    #[serde(flatten)]
    task: Task,
    parent: Option<ParentTask>,
}
