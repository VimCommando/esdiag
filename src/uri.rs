use crate::host::Host;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use url::Url;

#[derive(Debug)]
pub enum Uri {
    Host(Host),
    Url(Url),
    Directory(PathBuf),
    File(PathBuf),
    Stream,
}

pub fn classify(uri: &str) -> Uri {
    match uri {
        "-" => Uri::Stream,
        _ => {
            let host = Host::from_str(&uri);
            match host {
                Err(_) => log::debug!("No known host {uri}"),
                Ok(host) => return Uri::Host(host),
            }
            match Url::parse(&uri) {
                Err(_) => log::debug!("Not a valid URL {uri}"),
                Ok(url) => return Uri::Url(url),
            }
            match Path::new(&uri).is_dir() {
                false => log::debug!("Not a directory {uri}"),
                true => {
                    log::debug!("Directory {uri}");
                    return Uri::Directory(PathBuf::from_str(&uri).unwrap());
                }
            }
            match Path::new(&uri).is_file() {
                false => log::debug!("Not a file {uri}"),
                true => return Uri::File(PathBuf::from_str(&uri).unwrap()),
            }
            log::warn!("Unknown URI {uri}, falling back to stream");
            Uri::Stream
        }
    }
}
