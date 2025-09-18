use super::processor::{IndexStatsDocument, ShardStatsDocument};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test]
async fn deserialize_shard_documents_is_ok() {
    let file =
        File::open("src/processor/elasticsearch/indices_stats/tests/metrics-shard-esdiag.ndjson")
            .await
            .unwrap();
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut documents = Vec::new();

    while let Some(line) = lines.next_line().await.unwrap() {
        if !line.trim().is_empty() {
            let doc: ShardStatsDocument = serde_json::from_str(&line).unwrap();
            documents.push(doc);
        }
    }

    let result = Ok::<Vec<ShardStatsDocument>, std::io::Error>(documents);
    assert!(result.is_ok());
}

#[tokio::test]
async fn deserialize_index_documents_is_ok() {
    let file =
        File::open("src/processor/elasticsearch/indices_stats/tests/metrics-index-esdiag.ndjson")
            .await
            .unwrap();
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut documents = Vec::new();

    while let Some(line) = lines.next_line().await.unwrap() {
        if !line.trim().is_empty() {
            let doc: IndexStatsDocument = serde_json::from_str(&line).unwrap();
            documents.push(doc);
        }
    }

    let result = Ok::<Vec<IndexStatsDocument>, std::io::Error>(documents);
    assert!(result.is_ok());
}
