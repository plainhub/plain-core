use crate::features::downloads::Command;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::Result;
use std::collections::HashMap;
use tokio::{
    sync::{Mutex, watch},
    task::JoinHandle,
};

mod worker;
use worker::Worker;

pub struct Runtime {
    workers: Mutex<HashMap<String, Worker>>,
    job: Mutex<Option<JoinHandle<()>>>,
    stop: watch::Sender<bool>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            workers: Mutex::default(),
            job: Mutex::default(),
            stop: watch::channel(false).0,
        }
    }
}

impl Runtime {
    pub async fn control(&self, state: &CoreRuntime, id: &str, command: Command) -> Result<bool> {
        let mut workers = self.workers.lock().await;
        let changed = state.downloads.control(id, command)?;
        if changed {
            if let Some(worker) = workers.remove(id) {
                worker.stop().await?;
            }
        }
        Ok(changed)
    }
    pub async fn shutdown(&self) {
        self.stop.send_replace(true);
        if let Some(job) = self.job.lock().await.take() {
            if let Err(error) = job.await {
                log::error!("Download scheduler failed: {error}");
            }
        }
        for (_, worker) in self.workers.lock().await.drain() {
            if let Err(error) = worker.stop().await {
                log::error!("Download worker failed: {error}");
            }
        }
    }
}

pub fn start(state: CoreRuntime) {
    let runtime = state.download_runtime.clone();
    let owner = runtime.clone();
    let mut changed = state.downloads.changed.subscribe();
    let mut stop = state.stop.clone();
    let mut runtime_stop = runtime.stop.subscribe();
    let (completed, mut completions) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
    let job = tokio::spawn(async move {
        loop {
            {
                let mut workers = owner.workers.lock().await;
                while workers.len() < 3 {
                    let Some(run) = state.downloads.claim() else {
                        break;
                    };
                    let id = run.download.id.clone();
                    workers.insert(id, Worker::start(state.clone(), run, completed.clone()));
                }
            }
            tokio::select! {
                _ = runtime_stop.changed() => break,
                _ = stop.changed() => break,
                result = changed.changed() => {
                    if result.is_err() { break; }
                    state.host.invalidate("downloadsRefresh");
                    let _ = state.events.send(crate::infrastructure::events::WsEvent::broadcast(
                        crate::features::chat::events::WS_DOWNLOAD_PROGRESS,
                        "true".into(),
                    ));
                },
                Some((id, generation)) = completions.recv() => {
                    let worker = {
                        let mut workers = owner.workers.lock().await;
                        if workers.get(&id).is_some_and(|worker| worker.generation == generation) { workers.remove(&id) } else { None }
                    };
                    if let Some(worker) = worker {
                        if let Err(error) = worker.job.await.unwrap_or_else(|error| Err(error.into())) { log::error!("Download worker failed: {error}"); }
                    }
                }
            }
        }
        state.downloads.stop();
    });
    *runtime.job.try_lock().expect("New download scheduler") = Some(job);
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/downloads/runtime.rs"]
mod tests;
