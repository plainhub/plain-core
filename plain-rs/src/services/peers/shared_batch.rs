use anyhow::{Result, ensure};
use plain_rs::files::shares::batch_plan::Kind;
use plain_rs::files::shares::batch_plan::Plan;
use plain_rs::files::shares::batch_plan::Walker;
use plain_rs::files::shares::batch_queue::Queue;
use plain_rs::files::shares::batch_queue::Run;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::peers::shared_transfer::cleanup;
use plain_rs::services::peers::shared_transfer::operation;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{sync::Mutex, task::JoinHandle};

pub fn downloads_directory(host: &plain_rs::host::bridge::Host, target: &str) -> Result<String> {
    let directory = if target.is_empty() {
        let root = &host.startup().downloads_dir;
        ensure!(!root.is_empty(), "Downloads directory unavailable");
        std::path::Path::new(root).join("PlainApp")
    } else {
        std::path::PathBuf::from(target)
    };
    ensure!(
        directory.is_absolute() && !directory.to_string_lossy().contains('\0'),
        "Invalid downloads directory"
    );
    Ok(directory
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid downloads directory encoding"))?
        .to_owned())
}

#[derive(Default)]
pub struct Runtime {
    pub queue: Queue,
    workers: Mutex<HashMap<String, (String, Option<JoinHandle<()>>, Arc<AtomicBool>)>>,
}
impl Runtime {
    pub async fn shutdown(&self, directory: &std::path::Path) {
        self.queue.stop();
        let mut workers = self.workers.lock().await;
        for (_, (generation, job, clean)) in workers.drain() {
            if let Some(job) = job {
                job.abort();
                let _ = job.await;
            }
            if !clean.load(Ordering::SeqCst) {
                if let Err(error) = cleanup(directory, &generation).await {
                    log::error!("Shared transfer cleanup failed: {error}");
                }
            }
        }
    }
}
pub async fn control(state: &CoreRuntime, id: &str, command: &str) -> Result<bool> {
    ensure!(!*state.stop.borrow(), "Core stopped");
    let mut workers = state.shared_batches.workers.lock().await;
    if let Some((generation, worker, clean)) = workers.get_mut(id) {
        if !matches!(command, "pause" | "cancel" | "remove") {
            ensure!(
                !state.shared_batches.queue.is_active(id),
                "Shared batch still active"
            );
        }
        let paused = state.shared_batches.queue.control(id, "pause")?.is_some();
        if let Some(job) = worker.take() {
            job.abort();
            let _ = job.await;
        }
        if !clean.load(Ordering::SeqCst) {
            cleanup(&state.directory, generation).await?;
        }
        workers.remove(id);
        if command == "pause" {
            return Ok(paused);
        }
        if command == "retry" && paused {
            return Ok(state.shared_batches.queue.control(id, "resume")?.is_some());
        }
    }
    Ok(state.shared_batches.queue.control(id, command)?.is_some())
}
pub fn start(state: CoreRuntime) {
    let progress_state = state.clone();
    let mut changed = progress_state.shared_batches.queue.changed.subscribe();
    tokio::spawn(async move {
        let mut stop = progress_state.stop.clone();
        loop {
            tokio::select! { result=changed.changed()=>if result.is_err(){break}, _=stop.changed()=>break };
            progress_state
                .host
                .invalidate("sharedFolderDownloadsRefresh");
        }
    });

    let mut changed = state.shared_batches.queue.changed.subscribe();
    let mut stop = state.stop.clone();
    tokio::spawn(async move {
        loop {
            {
                let mut workers = state.shared_batches.workers.lock().await;
                workers.retain(|id, (_, _, clean)| {
                    !clean.load(Ordering::SeqCst) || state.shared_batches.queue.is_active(id)
                });
                while workers.len() < 3 {
                    let Some(run) = state
                        .shared_batches
                        .queue
                        .claim(&workers.keys().cloned().collect())
                    else {
                        break;
                    };
                    let id = run.snapshot.id.clone();
                    let generation = run.snapshot.generation.clone();
                    let worker_state = state.clone();
                    let cleaned = Arc::new(AtomicBool::new(false));
                    let worker_cleaned = cleaned.clone();
                    let job = tokio::spawn(async move {
                        let result = execute(&worker_state, &run).await;
                        let released =
                            cleanup(&worker_state.directory, &run.snapshot.generation).await;
                        worker_cleaned.store(released.is_ok(), Ordering::SeqCst);
                        let error = result
                            .err()
                            .map(|error| error.to_string())
                            .or_else(|| released.err().map(|error| error.to_string()));
                        worker_state.shared_batches.queue.finish(
                            &run.snapshot.id,
                            &run.snapshot.generation,
                            error,
                        );
                    });
                    workers.insert(id, (generation, Some(job), cleaned));
                }
            }
            tokio::select! {_=stop.changed()=>break, result=changed.changed()=>if result.is_err(){break}}
        }
        state.shared_batches.shutdown(&state.directory).await;
    });
}
async fn plan(state: &CoreRuntime, run: &Run) -> Result<Plan> {
    let mut walker = Walker::new(
        run.intent.kind,
        run.intent.entries.clone(),
        &run.intent.target_dir,
        &run.intent.downloads_base,
    )?;
    while let Some(path) = walker.next_directory()? {
        let info = plain_rs::services::peers::shared_client::fetch(
            state,
            &run.intent.link,
            if path.is_empty() { None } else { Some(&path) },
        )
        .await?;
        walker.supply(info.entries)?;
    }
    walker.finish()
}
async fn execute(state: &CoreRuntime, run: &Run) -> Result<()> {
    let plan = match &run.plan {
        Some(plan) => plan.clone(),
        None => plan(state, run).await?,
    };
    state
        .shared_batches
        .queue
        .planned(&run.snapshot.id, &run.snapshot.generation, plan.clone())?;
    let mut items = vec![];
    for target in &plan.targets {
        if run.intent.kind != Kind::Zip && run.completed.contains(&target.entry.virtual_path) {
            continue;
        }
        let url =
            run.intent
                .link
                .file_url(&run.intent.url_token, &target.entry.virtual_path, false)?;
        let result = operation(
            state,
            run,
            &target.entry.name,
            Some(target.entry.size),
            json!({"kind":"file","url":url,"target":target,"temporary":run.intent.kind==Kind::Zip}),
        )
        .await;
        match result {
            Ok(receipt) => {
                if run.intent.kind == Kind::Zip {
                    ensure!(
                        std::path::Path::new(&receipt.path).is_absolute(),
                        "Invalid OS temporary path"
                    );
                    items.push(json!({"sourcePath":receipt.path,"entryName":target.entry_name}));
                }
                state.shared_batches.queue.file_finished(
                    &run.snapshot.id,
                    &run.snapshot.generation,
                    &target.entry.virtual_path,
                    None,
                )?;
                if run.intent.kind != Kind::Zip {
                    state.file_platform.scan(&[receipt.path]).await?;
                }
            }
            Err(error) => {
                state.shared_batches.queue.file_finished(
                    &run.snapshot.id,
                    &run.snapshot.generation,
                    &target.entry.virtual_path,
                    Some(error.to_string()),
                )?;
                if run.intent.kind == Kind::Zip {
                    return Err(error);
                }
            }
        }
    }
    if run.intent.kind == Kind::Zip {
        ensure!(!items.is_empty(), "No shared files to archive");
        let receipt = operation(state,run,"",None,json!({"kind":"zip","items":items,"path":format!("{}/{}",run.intent.downloads_base.trim_end_matches('/'),run.intent.zip_name)})).await?;
        state.file_platform.scan(&[receipt.path]).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/shared_batch.rs"]
mod tests;
