use crate::{
    data::{
        diagnostic::{data_source::PathType, DataSource},
        elasticsearch::{
            AliasList, ClusterSettings, DataStreams, IlmExplain, IndicesSettings, IndicesStats,
            Nodes, NodesStats, SearchableSnapshotsCacheStats, SearchableSnapshotsStats, Tasks,
        },
    },
    exporter::DirectoryExporter,
    receiver::Receiver,
};
use color_eyre::eyre::{eyre, Result};
use std::path::PathBuf;

pub enum Collector {
    Elasticsearch(ElasticsearchCollector),
}

impl Collector {
    pub async fn try_new(receiver: Receiver, exporter: DirectoryExporter) -> Result<Self> {
        if let Receiver::Elasticsearch(_) = &receiver {
            let collector = ElasticsearchCollector::new(receiver, exporter).await?;
            Ok(Self::Elasticsearch(collector))
        } else {
            Err(eyre!(
                "Collect is only implemented from Elasticsearch to a Directory"
            ))
        }
    }

    pub async fn collect(&self) -> Result<usize> {
        match self {
            Self::Elasticsearch(collector) => collector.collect().await,
        }
    }
}

pub struct ElasticsearchCollector {
    receiver: Receiver,
    exporter: DirectoryExporter,
}

impl ElasticsearchCollector {
    pub async fn new(receiver: Receiver, exporter: DirectoryExporter) -> Result<Self> {
        Ok(Self { receiver, exporter })
    }

    pub async fn collect(&self) -> Result<usize> {
        self.save(self.receiver.try_get_manifest().await?).await?;
        self.save(self.receiver.get::<AliasList>().await?).await?;
        self.save(self.receiver.get::<ClusterSettings>().await?)
            .await?;
        self.save(self.receiver.get::<DataStreams>().await?).await?;
        self.save(self.receiver.get::<IlmExplain>().await?).await?;
        self.save(self.receiver.get::<IndicesSettings>().await?)
            .await?;
        self.save(self.receiver.get::<IndicesStats>().await?)
            .await?;
        self.save(self.receiver.get::<Nodes>().await?).await?;
        self.save(self.receiver.get::<NodesStats>().await?).await?;
        self.save(self.receiver.get::<SearchableSnapshotsCacheStats>().await?)
            .await?;
        match self.receiver.get::<SearchableSnapshotsStats>().await {
            Ok(stats) => self.save(stats).await?,
            Err(_) => log::warn!("No searchable snapshots stats available"),
        };
        self.save(self.receiver.get::<Tasks>().await?).await?;

        let file_count = 1;
        log::info!("Collected {file_count} files into {}", self.exporter);
        Ok(file_count)
    }

    async fn save<T>(&self, content: T) -> Result<()>
    where
        T: serde::Serialize + DataSource,
    {
        let path = PathBuf::from(T::source(PathType::File)?);
        self.exporter.save(path, content).await?;
        Ok(())
    }
}
