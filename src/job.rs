// Copyright Elasticsearch B.V. and/or licensed to Elasticsearch B.V. under one
// or more contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.

//! Shared job runner for executing saved diagnostic jobs.
//! Used by both the CLI (`esdiag job run`) and the web server.

use crate::{
    data::{CollectMode, KnownHost, ProcessMode, SendMode, Uri, Workflow},
    exporter::Exporter,
    processor::{Collector, Identifiers, Processor},
    receiver::Receiver,
};
use eyre::{Result, eyre};
use std::{path::PathBuf, sync::Arc};

pub async fn run_saved_job(
    workflow: &Workflow,
    identifiers: Identifiers,
    host: KnownHost,
) -> Result<()> {
    let host_url = host.get_url().to_string();
    tracing::info!("Running saved job against {host_url}");

    let need_collect = workflow.collect.mode == CollectMode::Collect;
    let need_process = workflow.process.enabled
        && workflow.process.mode == ProcessMode::Process;

    if need_collect && need_process {
        // Collect → Process → Send
        let temp_dir = std::env::temp_dir().join(format!(
            "esdiag-job-{}",
            uuid::Uuid::new_v4().as_u64_pair().0
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let _cleanup = TempDirCleanup(temp_dir.clone());

        tracing::info!("Collecting diagnostic from {host_url}");
        let product = host.app().clone();
        let diagnostic_type = workflow.collect.diagnostic_type.clone();
        let receiver = Receiver::try_from(host)?;
        let collect_exporter = Exporter::for_collect_archive(temp_dir)?;
        let collector = Collector::try_new(
            receiver,
            collect_exporter,
            product,
            diagnostic_type,
            None,
            None,
            identifiers.clone(),
        )
        .await?;
        let result = collector.collect().await?;
        let archive_path = PathBuf::from(result.path);
        tracing::info!("Collected archive: {}", archive_path.display());

        // Process the collected archive
        let exporter = resolve_exporter(workflow)?;
        let receiver = Arc::new(Receiver::try_from(Uri::File(archive_path))?);
        let exporter = Arc::new(exporter);
        let processor = Processor::try_new(receiver, exporter, identifiers).await?;
        let processor = processor
            .start()
            .await
            .map_err(|failed| eyre!("{}", failed))?;
        match processor.process().await {
            Ok(completed) => {
                tracing::info!(
                    "Processing complete in {:.3}s",
                    completed.state.runtime as f64 / 1000.0
                );
                Ok(())
            }
            Err(failed) => Err(eyre!("{}", failed)),
        }
    } else if need_collect {
        // Collect only (save to disk)
        let save_dir = if workflow.collect.save_dir.is_empty() {
            std::env::current_dir()?
        } else {
            PathBuf::from(&workflow.collect.save_dir)
        };
        tracing::info!("Collecting diagnostic from {host_url}");
        let product = host.app().clone();
        let diagnostic_type = workflow.collect.diagnostic_type.clone();
        let receiver = Receiver::try_from(host)?;
        let collect_exporter = Exporter::for_collect_archive(save_dir)?;
        let collector = Collector::try_new(
            receiver,
            collect_exporter,
            product,
            diagnostic_type,
            None,
            None,
            identifiers,
        )
        .await?;
        let result = collector.collect().await?;
        tracing::info!("Collected archive: {}", result.path);
        Ok(())
    } else {
        Err(eyre!("Saved job has no valid execution path"))
    }
}

fn resolve_exporter(workflow: &Workflow) -> Result<Exporter> {
    match workflow.send.mode {
        SendMode::Remote => {
            let target = workflow.send.remote_target.trim();
            if target.is_empty() {
                return Err(eyre!("Remote send target is empty"));
            }
            Exporter::try_from(Uri::try_from(target.to_string())?)
        }
        SendMode::Local => {
            let target = workflow.send.local_target.trim();
            if target == "directory" {
                let directory = workflow.send.local_directory.trim();
                if directory.is_empty() {
                    return Err(eyre!(
                        "Local directory output requires a directory path"
                    ));
                }
                Exporter::try_from(Uri::try_from(directory.to_string())?)
            } else if target.is_empty() {
                Err(eyre!(
                    "Local send requires a target"
                ))
            } else {
                Exporter::try_from(Uri::try_from(target.to_string())?)
            }
        }
    }
}

struct TempDirCleanup(PathBuf);

impl Drop for TempDirCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
