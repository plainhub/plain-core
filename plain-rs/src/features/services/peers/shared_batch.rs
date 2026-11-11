use crate::features::files::shares::batch_plan::Kind;
use crate::features::files::shares::batch_plan::Plan;
use crate::features::files::shares::batch_plan::Walker;
use crate::features::files::shares::batch_queue::Queue;
use crate::features::files::shares::batch_queue::Run;
use crate::features::services::peers::shared_transfer::operation;
use crate::features::services::peers::shared_transfer::{BlockingWork, cleanup};
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{sync::Mutex, task::JoinHandle};

pub fn downloads_directory(
    host: &crate::infrastructure::host::bridge::Host,
    target: &str,
) -> Result<String> {
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
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid downloads directory encoding"))?
        .to_owned())
}

struct Worker {
    generation: String,
    task: Option<JoinHandle<()>>,
    clean: Arc<AtomicBool>,
    io: Arc<BlockingWork>,
}

#[derive(Default)]
pub struct Runtime {
    pub queue: Queue,
    workers: Mutex<HashMap<String, Worker>>,
}
impl Runtime {
    pub async fn shutdown(&self, directory: &std::path::Path) {
        self.queue.stop();
        let mut workers = self.workers.lock().await;
        for (_, worker) in workers.drain() {
            if let Some(job) = worker.task {
                job.abort();
                let _ = job.await;
            }
            if let Err(error) = worker.io.finish().await {
                log::error!("Shared transfer I/O failed: {error}");
            }
            if !worker.clean.load(Ordering::SeqCst) {
                if let Err(error) = cleanup(directory, &worker.generation).await {
                    log::error!("Shared transfer cleanup failed: {error}");
                }
            }
        }
    }
}
pub async fn control(state: &CoreRuntime, id: &str, command: &str) -> Result<bool> {
    ensure!(!*state.stop.borrow(), "Core stopped");
    let mut workers = state.shared_batches.workers.lock().await;
    if let Some(worker) = workers.get_mut(id) {
        if !matches!(command, "pause" | "cancel" | "remove") {
            ensure!(
                !state.shared_batches.queue.is_active(id),
                "Shared batch still active"
            );
        }
        let paused = state.shared_batches.queue.control(id, "pause")?.is_some();
        if let Some(job) = worker.task.as_mut() {
            job.abort();
            let _ = job.await;
        }
        worker.task = None;
        let finished = worker.io.finish().await;
        if !worker.clean.load(Ordering::SeqCst) {
            cleanup(&state.directory, &worker.generation).await?;
        }
        finished?;
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
                workers.retain(|id, worker| {
                    !worker.clean.load(Ordering::SeqCst) || state.shared_batches.queue.is_active(id)
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
                    let io = Arc::new(BlockingWork::default());
                    let worker_io = io.clone();
                    let job = tokio::spawn(async move {
                        let result = execute(&worker_state, &worker_io, &run).await;
                        let finished = worker_io.finish().await;
                        let released =
                            cleanup(&worker_state.directory, &run.snapshot.generation).await;
                        worker_cleaned.store(released.is_ok(), Ordering::SeqCst);
                        let error = result
                            .err()
                            .map(|error| error.to_string())
                            .or_else(|| finished.err().map(|error| error.to_string()))
                            .or_else(|| released.err().map(|error| error.to_string()));
                        worker_state.shared_batches.queue.finish(
                            &run.snapshot.id,
                            &run.snapshot.generation,
                            error,
                        );
                    });
                    workers.insert(
                        id,
                        Worker {
                            generation,
                            task: Some(job),
                            clean: cleaned,
                            io,
                        },
                    );
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
        let info = crate::features::services::peers::shared_client::fetch(
            state,
            &run.intent.link,
            if path.is_empty() { None } else { Some(&path) },
        )
        .await?;
        walker.supply(info.entries)?;
    }
    walker.finish()
}
async fn execute(state: &CoreRuntime, work: &BlockingWork, run: &Run) -> Result<()> {
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
        if run.intent.kind != Kind::Zip && run.completed.contains(&target.entry.relative_path) {
            continue;
        }
        let url =
            run.intent
                .link
                .file_url(&run.intent.url_token, &target.entry.relative_path, false)?;
        let url = format!("{url}&action=download");
        let result = operation(
            state,
            work,
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
                    &target.entry.relative_path,
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
                    &target.entry.relative_path,
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
        let receipt = operation(state,work,run,"",None,json!({"kind":"zip","items":items,"path":format!("{}/{}",run.intent.downloads_base.trim_end_matches('/'),run.intent.zip_name)})).await?;
        state.file_platform.scan(&[receipt.path]).await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/shared_batch.rs"]
mod tests;
