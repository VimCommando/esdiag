use super::Receive;
use crate::data::{diagnostic::data_source::DataSource, Uri};
use color_eyre::eyre::{eyre, Result};
use serde::de::DeserializeOwned;
use std::{fs::File, io::BufReader, path::PathBuf};

pub struct DirectoryReceiver {
    path: PathBuf,
    uri: Uri,
}

impl TryFrom<Uri> for DirectoryReceiver {
    type Error = color_eyre::eyre::Report;

    fn try_from(uri: Uri) -> Result<Self> {
        match uri {
            Uri::Directory(ref path) => match path.is_dir() {
                true => {
                    log::debug!("Directory is valid: {}", path.display());
                    Ok(Self {
                        path: path.clone(),
                        uri,
                    })
                }
                false => {
                    log::debug!("Directory is invalid: {}", path.display());
                    Err(eyre!(
                        "Directory input must be a directory: {}",
                        path.display()
                    ))
                }
            },
            _ => Err(eyre!("URI is not a directory")),
        }
    }
}

impl Receive for DirectoryReceiver {
    async fn is_connected(&self) -> bool {
        let is_dir = self.path.is_dir();
        let directory_name = self.path.to_str().unwrap_or("");
        log::debug!("Directory {directory_name} is valid: {is_dir}");
        is_dir
    }

    async fn get<T>(&self) -> Result<T>
    where
        T: DeserializeOwned + DataSource,
    {
        let filename = &self.path.join(T::source(&self.uri)?);
        log::debug!("Reading file: {}", &filename.display());
        let file = File::open(&filename)?;
        let reader = BufReader::new(file);
        let data: T = serde_json::from_reader(reader)?;
        Ok(data)
    }
}

impl std::fmt::Display for DirectoryReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.path.display())
    }
}
