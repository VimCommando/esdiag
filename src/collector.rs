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
        let total = 12;
        let mut file_count = 0;
        //self.save(self.receiver.try_get_manifest().await?).await?;
        file_count += self.save::<AliasList>().await?;
        file_count += self.save::<ClusterSettings>().await?;
        file_count += self.save::<DataStreams>().await?;
        file_count += self.save::<IlmExplain>().await?;
        file_count += self.save::<IndicesSettings>().await?;
        file_count += self.save::<IndicesStats>().await?;
        file_count += self.save::<Nodes>().await?;
        file_count += self.save::<NodesStats>().await?;
        file_count += self.save::<SearchableSnapshotsCacheStats>().await?;
        file_count += self.save::<SearchableSnapshotsStats>().await?;
        file_count += self.save::<Tasks>().await?;

        log::info!(
            "Collected {file_count} of {total} files into {}",
            self.exporter
        );
        Ok(file_count)
    }

    async fn save<T>(&self) -> Result<usize>
    where
        T: DataSource,
    {
        let content = self.receiver.get_raw::<T>().await?;
        let path = PathBuf::from(T::source(PathType::File)?);
        self.exporter.save(path, content).await.map(|_| 1)
    }
}
