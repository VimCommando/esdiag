/// Read from a `.zip` archive file
pub mod archive;
/// Read from a direcotry in the local file system
pub mod directory;
/// Request API calls from Elasticsearch
pub mod elasticsearch;
/// Read from a file the local file system
pub mod file;

use crate::data::diagnostic::{
    data_source::{DataSource, Source},
    DataSet, ElasticCloudKubernetes, Elasticsearch, Kibana, Logstash, Manifest, Product,
};
use crate::data::Uri;
use archive::ArchiveReceiver;
use color_eyre::eyre::{eyre, Result};
use directory::DirectoryReceiver;
use elasticsearch::ElasticsearchReceiver;
use semver::Version;
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;

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

// Input struct to hold the product, sources, and version

#[derive(Debug)]
pub struct Input {
    pub dataset: InputDataSets,
    pub product: Product,
    pub sources: BTreeMap<String, Source>,
    pub uri: Uri,
    pub version: Option<Version>,
    pub manifest: Manifest,
}

impl Input {
    pub fn new(uri: Uri, manifest: Manifest) -> Self {
        let application = match &manifest.product {
            Product::Agent => todo!("Elastic Agent"),
            Product::ECE => todo!("Elasitc Cloud Enterprise (ECE)"),
            Product::ECK => ElasticCloudKubernetes::new(),
            Product::Elasticsearch => Elasticsearch::new(),
            Product::Kibana => Kibana::new(),
            Product::Logstash => Logstash::new(),
            Product::Unknown => panic!("Cannot import an unknown product!"),
        };
        let sources = match file::parse_sources_yml(&manifest.product) {
            Ok(sources) => sources,
            Err(e) => panic!("Error parsing sources file: {}", e),
        };
        let version = match &manifest.product_version {
            Some(product_version) => Version::new(
                product_version.major,
                product_version.minor,
                product_version.patch,
            ),
            None => Version::new(0, 0, 0),
        };

        Self {
            product: manifest.product.clone(),
            dataset: InputDataSets {
                data: application.get_data_sets(),
                lookup: application.get_lookup_sets(),
                metadata: application.get_metadata_sets(),
            },
            manifest,
            uri,
            sources,
            version: Some(version),
        }
    }

    pub fn get_source(&self, dataset: &DataSet) -> Option<&Source> {
        let name = dataset.to_string();
        self.sources.get(&name)
    }

    pub fn load_string(&self, dataset: &DataSet) -> Option<String> {
        let name = dataset.to_string();
        let source = match self.sources.get(&name) {
            Some(source) => source,
            None => panic!("ERROR: Source not found for {name}"),
        };
        match &self.uri {
            Uri::Directory(dir) => {
                match file::read_string(&dir.with_file_name(&source.as_path_string(&name))) {
                    Ok(string) => Some(string),
                    Err(e) => {
                        log::debug!("Error reading file '{:?}'", e);
                        None
                    }
                }
            }
            Uri::File(file) => match archive::read_string(file, &source.as_path_string(&name)) {
                Ok(string) => Some(string),
                Err(e) => {
                    log::debug!("Error reading file '{:?}'", e);
                    None
                }
            },
            _ => {
                unimplemented!("Only Directory and File input types implemented!");
            }
        }
    }
}

impl std::fmt::Display for Input {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            fmt,
            "Processing {} version {} from {:?}",
            self.product,
            self.version.clone().unwrap(),
            self.uri,
        )
    }
}
