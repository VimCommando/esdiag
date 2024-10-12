/// Read from a `.zip` archive file
mod archive;
/// Read from a direcotry in the local file system
mod directory;
/// Request API calls from Elasticsearch
mod elasticsearch;

use archive::ArchiveReceiver;
use directory::DirectoryReceiver;
use elasticsearch::ElasticsearchReceiver;

use crate::data::{diagnostic::data_source::DataSource, Uri};
use color_eyre::eyre::{eyre, Result};
use serde::de::DeserializeOwned;

trait Receive {
    #[allow(dead_code)]
    async fn is_connected(&self) -> bool;
    async fn get<T>(&self) -> Result<T>
    where
        T: DataSource + DeserializeOwned;
}

/// The different types of receivers for data input.
///
/// This enum encapsulates various implementations of the `Receive` trait,
/// allowing for flexible handling of different data sources. Each variant
/// corresponds to a specific method of data retrieval:
///
/// - `Archive`: Reads data from a `.zip` archive file.
/// - `Directory`: Reads data from a directory in the local file system.
/// - `Elasticsearch`: Requests data via API calls from an Elasticsearch service.
pub enum Receiver {
    /// Read from a `.zip` archive file
    Archive(ArchiveReceiver),
    /// Read from a direcotry in the local file system
    Directory(DirectoryReceiver),
    /// Request API calls from Elasticsearch
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
