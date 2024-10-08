/// Read from a `.zip` archive file
mod archive;
/// Read from a direcotry in the local file system
mod directory;
/// Request API calls from Elasticsearch
mod elasticsearch;
/// Read from a file the local file system (not for diagnostic bundles)
pub mod file;

use crate::data::diagnostic::{
    data_source::{DataSource, Source},
    DataSet,
};
use crate::data::Uri;
use archive::ArchiveReceiver;
use color_eyre::eyre::{eyre, Result};
use directory::DirectoryReceiver;
use elasticsearch::ElasticsearchReceiver;
use serde::de::DeserializeOwned;

trait Receive {
    #[allow(dead_code)]
    async fn is_connected(&self) -> bool;
    async fn get<T>(&self) -> Result<T>
    where
        T: DataSource + DeserializeOwned;
}

pub enum Receiver {
    Archive(ArchiveReceiver),
    Directory(DirectoryReceiver),
    Elasticsearch(ElasticsearchReceiver),
}

impl Receiver {
    pub async fn get<T>(&self) -> Result<T>
    where
        T: DataSource + DeserializeOwned,
    {
        match self {
            Receiver::Archive(archive_receiver) => archive_receiver.get::<T>().await,
            Receiver::Directory(directory_receiver) => directory_receiver.get::<T>().await,
            Receiver::Elasticsearch(elasticsearch_receiver) => {
                elasticsearch_receiver.get::<T>().await
            }
        }
    }
}

impl TryFrom<Uri> for Receiver {
    type Error = color_eyre::Report;
    fn try_from(uri: Uri) -> std::result::Result<Self, Self::Error> {
        match uri {
            Uri::Directory(_) => Ok(Receiver::Directory(DirectoryReceiver::try_from(uri)?)),
            Uri::File(_) => Ok(Receiver::Archive(ArchiveReceiver::try_from(uri)?)),
            Uri::Host(_) => Ok(Receiver::Elasticsearch(ElasticsearchReceiver::try_from(
                uri,
            )?)),
            _ => Err(eyre!("Unsupported URI")),
        }
    }
}

impl std::fmt::Display for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Receiver::Archive(archive_receiver) => write!(f, "file {}", archive_receiver),
            Receiver::Directory(directory_receiver) => write!(f, "file {}", directory_receiver),
            Receiver::Elasticsearch(elasticsearch_receiver) => {
                write!(f, "elasticsearch {}", elasticsearch_receiver)
            }
        }
    }
}

#[derive(Debug)]
pub struct InputDataSets {
    pub data: Vec<DataSet>,
    pub lookup: Vec<DataSet>,
    pub metadata: Vec<DataSet>,
}

impl InputDataSets {
    pub fn len(&self) -> usize {
        &self.data.len() + &self.lookup.len() + &self.metadata.len()
    }
}

impl std::fmt::Display for InputDataSets {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            fmt,
            "Data: [{}], Lookup: [{}], Metadata: [{}]",
            self.data
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            self.lookup
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            self.metadata
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
