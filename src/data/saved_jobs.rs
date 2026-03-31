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
    let temp_path = path.with_extension("yml.tmp");
    fs::write(&temp_path, content)?;
    fs::rename(&temp_path, &path)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_env_lock;
    use tempfile::TempDir;

    fn setup_env() -> TempDir {
        let tmp = TempDir::new().expect("temp dir");
        let hosts = tmp.path().join("hosts.yml");
        unsafe {
            std::env::set_var("ESDIAG_HOSTS", &hosts);
        }
        tmp
    }

    #[test]
    fn save_saved_jobs_overwrites_existing_file() {
        let _guard = test_env_lock().lock().expect("env lock");
        let _tmp = setup_env();

        let mut jobs = SavedJobs::default();
        jobs.insert("first".to_string(), SavedJob::default());
        save_saved_jobs(&jobs).expect("save initial jobs");

        let mut updated_jobs = SavedJobs::default();
        updated_jobs.insert("second".to_string(), SavedJob::default());
        save_saved_jobs(&updated_jobs).expect("overwrite jobs");

        let loaded_jobs = load_saved_jobs().expect("load saved jobs");
        assert!(loaded_jobs.contains_key("second"));
        assert!(!loaded_jobs.contains_key("first"));
    }
}
