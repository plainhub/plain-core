use super::{Command, Download, Failure, Source, Status};
mod lifecycle;
mod progress;
use plain_libs::anyhow::{Result, ensure};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Mutex,
    time::Instant,
};
use tokio::sync::watch;

#[derive(Clone)]
pub struct Run {
    pub download: Download,
    pub generation: String,
    pub completed: HashMap<String, i64>,
    pub peers: Vec<crate::infrastructure::db::DPeer>,
    pub attachments: Vec<super::Attachment>,
    pub attempt: crate::infrastructure::transport::router::Attempt,
}
struct Task {
    run: Run,
    current_size: i64,
    current_bytes: i64,
    last_bytes: i64,
    last: Instant,
}
#[derive(Default)]
struct State {
    tasks: BTreeMap<String, Task>,
    pending: VecDeque<String>,
    stopped: bool,
}
pub struct Queue {
    state: Mutex<State>,
    pub changed: watch::Sender<u64>,
}
impl Default for Queue {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            changed: watch::channel(0).0,
        }
    }
}
fn matching<'a>(
    state: &'a State,
    source: &Source,
    target_dir: &str,
    zip_name: Option<&str>,
) -> Result<Option<&'a Task>> {
    if let Some(task) = state.tasks.values().find(|task| {
        &task.run.download.source == source
            && task.run.download.target_dir == target_dir
            && task.run.download.zip_name.as_deref() == zip_name
            && !task.run.download.status.terminal()
    }) {
        return Ok(Some(task));
    }
    if let Source::Chat {
        message_id,
        peer_id,
        file_ids,
    } = source
    {
        for task in state.tasks.values() {
            if let Source::Chat {
                message_id: old_message,
                peer_id: old_peer,
                file_ids: old_files,
            } = &task.run.download.source
            {
                if old_message == message_id && old_peer == peer_id {
                    if file_ids.iter().all(|id| old_files.contains(id)) {
                        return Ok(Some(task));
                    }
                    ensure!(
                        !file_ids.iter().any(|id| old_files.contains(id)),
                        "Attachment already queued"
                    );
                }
            }
        }
    }
    Ok(None)
}

fn capacity(state: &State) -> Result<()> {
    ensure!(
        state
            .tasks
            .values()
            .filter(|task| !task.run.download.status.terminal())
            .count()
            < 128,
        "Download task capacity exceeded"
    );
    Ok(())
}

impl Queue {
    fn notify(&self) {
        self.changed
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }
    pub fn existing(
        &self,
        source: &Source,
        target_dir: &str,
        zip_name: Option<&str>,
    ) -> Result<Option<Download>> {
        let state = self.state.lock().unwrap();
        Ok(matching(&state, source, target_dir, zip_name)?.map(|task| task.run.download.clone()))
    }
    pub fn create(
        &self,
        source: Source,
        target_dir: String,
        zip_name: Option<String>,
        title: String,
        peers: Vec<crate::infrastructure::db::DPeer>,
        attachments: Vec<super::Attachment>,
    ) -> Result<Download> {
        let mut state = self.state.lock().unwrap();
        ensure!(!state.stopped, "Downloads stopped");
        if let Some(task) = matching(&state, &source, &target_dir, zip_name.as_deref())? {
            return Ok(task.run.download.clone());
        }
        capacity(&state)?;
        let download = Download {
            id: crate::features::record_id::generate(),
            title,
            source,
            target_dir,
            zip_name,
            status: Status::Pending,
            downloaded_bytes: 0,
            total_bytes: 0,
            bytes_per_second: 0,
            total_files: 0,
            done_files: 0,
            current_file: String::new(),
            error: String::new(),
            failures: vec![],
        };
        state.tasks.insert(
            download.id.clone(),
            Task {
                run: Run {
                    download: download.clone(),
                    generation: String::new(),
                    completed: HashMap::new(),
                    peers,
                    attachments,
                    attempt: crate::infrastructure::transport::router::Attempt::Automatic,
                },
                current_size: 0,
                current_bytes: 0,
                last_bytes: 0,
                last: Instant::now(),
            },
        );
        state.pending.push_back(download.id.clone());
        self.notify();
        Ok(download)
    }
    pub fn snapshot(&self) -> Vec<Download> {
        self.state
            .lock()
            .unwrap()
            .tasks
            .values()
            .map(|task| task.run.download.clone())
            .collect()
    }
    pub fn claim(&self) -> Option<Run> {
        let mut state = self.state.lock().unwrap();
        if state.stopped {
            return None;
        }
        let id = state.pending.pop_front()?;
        let task = state.tasks.get_mut(&id).expect("Pending download exists");
        task.run.generation = plain_libs::utils::uuid::Uuid::new_v4().into_string();
        task.run.download.status = Status::Downloading;
        task.run.download.error.clear();
        task.run.download.failures.clear();
        task.run.download.downloaded_bytes = task.run.completed.values().sum();
        task.last_bytes = task.run.download.downloaded_bytes;
        task.last = Instant::now();
        let run = task.run.clone();
        task.run.attempt = crate::infrastructure::transport::router::Attempt::Automatic;
        self.notify();
        Some(run)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/downloads/queue.rs"]
mod tests;
