use eyre::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use super::workflow::Workflow;
use crate::processor::Identifiers;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SavedJob {
    #[serde(default)]
    pub identifiers: Identifiers,
    #[serde(default)]
    pub workflow: Workflow,
}

pub type SavedJobs = IndexMap<String, SavedJob>;

pub fn load_saved_jobs() -> Result<SavedJobs> {
    let path = get_jobs_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        if content.trim().is_empty() {
            return Ok(SavedJobs::default());
        }
        let jobs: SavedJobs = serde_yaml::from_str(&content)?;
        Ok(jobs)
    } else {
        Ok(SavedJobs::default())
    }
}

pub fn save_saved_jobs(jobs: &SavedJobs) -> Result<()> {
    let path = get_jobs_path()?;
    let content = serde_yaml::to_string(jobs)?;
    fs::write(path, content)?;
    Ok(())
}

fn get_jobs_path() -> Result<PathBuf> {
    let hosts_path = super::KnownHost::get_hosts_path();
    let esdiag_dir = hosts_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    if !esdiag_dir.exists() {
        fs::create_dir_all(&esdiag_dir)?;
    }
    Ok(esdiag_dir.join("jobs.yml"))
}
