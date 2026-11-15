use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Running,
    Paused,
    Canceling,
    Canceled,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCoverage {
    pub path: String,
    pub state: CoverageState,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageState {
    Pending,
    Scanning,
    Completed,
    Excluded,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub volume_id: Option<String>,
    pub status: Status,
    pub total_files: Option<usize>,
    pub processed_files: usize,
    pub error: Option<String>,
    pub sources: Vec<SourceCoverage>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Purpose {
    Rebuild,
    Reconcile,
}

struct Job {
    purpose: Purpose,
    snapshot: Snapshot,
    cancel: watch::Sender<bool>,
    activity: watch::Sender<()>,
    task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Default)]
struct State {
    defaults: crate::features::files::index_scope::Defaults,
    jobs: Vec<Job>,
    closed: bool,
    policy: Option<Arc<crate::features::services::files::media_scan_policy::Policy>>,
}

impl State {
    fn rebuild_snapshot(&self) -> Option<&Snapshot> {
        self.jobs
            .iter()
            .rev()
            .filter(|job| job.purpose == Purpose::Rebuild)
            .min_by_key(|job| match job.snapshot.status {
                Status::Running | Status::Canceling => 0,
                Status::Queued => 1,
                Status::Paused => 2,
                _ => 3,
            })
            .map(|job| &job.snapshot)
    }

    fn schedule(&mut self) {
        if self.closed
            || self
                .jobs
                .iter()
                .any(|job| matches!(job.snapshot.status, Status::Running | Status::Canceling))
        {
            return;
        }
        if let Some(job) = self
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.status == Status::Queued)
        {
            job.snapshot.status = Status::Running;
            job.activity.send_replace(());
        }
    }
}

#[derive(Default)]
pub struct MediaScans {
    state: Mutex<State>,
    events: Option<tokio::sync::broadcast::Sender<crate::infrastructure::events::WsEvent>>,
}

impl MediaScans {
    pub fn new(
        events: tokio::sync::broadcast::Sender<crate::infrastructure::events::WsEvent>,
        cache_directory: std::path::PathBuf,
    ) -> Self {
        Self {
            state: Mutex::new(State {
                policy: Some(Arc::new(
                    crate::features::services::files::media_scan_policy::Policy {
                        cache_directory,
                        excluded_roots: Vec::new(),
                        scope: None,
                    },
                )),
                ..State::default()
            }),
            events: Some(events),
        }
    }

    fn publish(&self, state: &State, id: &str) {
        if let Some(job) = state
            .jobs
            .iter()
            .find(|job| job.snapshot.id == id && job.purpose == Purpose::Rebuild)
        {
            self.publish_snapshot(&job.snapshot);
        }
        if let Some(snapshot) = state.rebuild_snapshot()
            && snapshot.id != id
            && snapshot.status == Status::Running
        {
            self.publish_snapshot(snapshot);
        }
    }

    fn publish_snapshot(&self, snapshot: &Snapshot) {
        let Some(events) = &self.events else {
            return;
        };
        let state = match snapshot.status {
            Status::Queued => "QUEUED",
            Status::Running => "RUNNING",
            Status::Paused => "PAUSED",
            Status::Canceling | Status::Canceled => "STOPPED",
            Status::Completed => "IDLE",
            Status::Failed => "FAILED",
        };
        let payload = plain_libs::serde_json::json!({
            "id": snapshot.id,
            "volumeId": snapshot.volume_id,
            "indexed": snapshot.processed_files,
            "pending": snapshot.total_files.map(|total| total.saturating_sub(snapshot.processed_files)),
            "total": snapshot.total_files,
            "state": state,
            "error": snapshot.error,
        });
        let _ = events.send(crate::infrastructure::events::WsEvent::broadcast(
            crate::infrastructure::events::MEDIA_SCAN_PROGRESS,
            payload.to_string(),
        ));
    }

    pub fn begin(self: &Arc<Self>, total_files: usize) -> Result<Session, String> {
        self.begin_with_total(Some(total_files), Purpose::Rebuild)
    }

    pub fn begin_discovery(self: &Arc<Self>, purpose: Purpose) -> Result<Session, String> {
        self.begin_with_total(None, purpose)
    }

    fn begin_with_total(
        self: &Arc<Self>,
        total_files: Option<usize>,
        purpose: Purpose,
    ) -> Result<Session, String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        if guard.jobs.iter().any(|job| {
            matches!(
                job.snapshot.status,
                Status::Queued | Status::Running | Status::Paused | Status::Canceling
            )
        }) {
            return Err("Media scan already running".into());
        }
        guard
            .jobs
            .retain(|job| purpose == Purpose::Reconcile && job.purpose == Purpose::Rebuild);
        Ok(self.insert_job(&mut guard, None, total_files, purpose))
    }

    pub fn enqueue_volume(self: &Arc<Self>, volume_id: &str) -> Result<Option<Session>, String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        if guard.jobs.iter().any(|job| {
            job.snapshot.volume_id.as_deref() == Some(volume_id)
                && matches!(
                    job.snapshot.status,
                    Status::Queued | Status::Running | Status::Paused | Status::Canceling
                )
        }) {
            return Ok(None);
        }
        guard
            .jobs
            .retain(|job| job.snapshot.volume_id.as_deref() != Some(volume_id));
        Ok(Some(self.insert_job(
            &mut guard,
            Some(volume_id.to_owned()),
            None,
            Purpose::Rebuild,
        )))
    }

    fn insert_job(
        self: &Arc<Self>,
        guard: &mut State,
        volume_id: Option<String>,
        total_files: Option<usize>,
        purpose: Purpose,
    ) -> Session {
        let (cancel, receiver) = watch::channel(false);
        let (activity, _) = watch::channel(());
        let id = plain_libs::utils::uuid::Uuid::new_v4().into_string();
        guard.jobs.push(Job {
            purpose,
            snapshot: Snapshot {
                id: id.clone(),
                volume_id,
                status: Status::Queued,
                total_files,
                processed_files: 0,
                error: None,
                sources: Vec::new(),
            },
            cancel,
            activity,
            task: None,
        });
        guard.schedule();
        self.publish(guard, &id);
        Session {
            scans: self.clone(),
            id,
            cancel: receiver,
        }
    }

    pub fn spawn<F>(
        &self,
        runtime: &tokio::runtime::Handle,
        session: Session,
        run: impl FnOnce(Session) -> F,
    ) -> Result<Snapshot, String>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        let snapshot = guard
            .jobs
            .iter()
            .find(|job| job.snapshot.id == session.id)
            .ok_or("Media scan replaced")?
            .snapshot
            .clone();
        let task = runtime.spawn(run(session));
        guard
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == snapshot.id)
            .ok_or("Media scan replaced")?
            .task = Some(task);
        Ok(snapshot)
    }

    pub async fn wait_background(&self) -> Result<(), String> {
        let tasks = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .jobs
            .iter_mut()
            .filter_map(|job| job.task.take())
            .collect::<Vec<_>>();
        for task in tasks {
            task.await.map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub async fn wait_volume(&self, volume_id: &str) -> Result<(), String> {
        let task = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.volume_id.as_deref() == Some(volume_id))
            .and_then(|job| job.task.take());
        if let Some(task) = task {
            task.await.map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn defaults(&self) -> Result<crate::features::files::index_scope::Defaults, String> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .defaults
            .clone())
    }

    pub fn set_defaults(
        &self,
        defaults: crate::features::files::index_scope::Defaults,
    ) -> Result<(), String> {
        self.state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .defaults = defaults;
        Ok(())
    }

    pub async fn wait_idle(&self) -> Result<bool, String> {
        let mut completed = true;
        loop {
            let activity = self
                .state
                .lock()
                .map_err(|_| "Media scan state unavailable")?
                .jobs
                .iter()
                .find(|job| {
                    matches!(
                        job.snapshot.status,
                        Status::Queued | Status::Running | Status::Paused | Status::Canceling
                    )
                })
                .map(|job| (job.activity.subscribe(), job.cancel.subscribe()));
            let Some((mut activity, canceled)) = activity else {
                return Ok(completed);
            };
            activity
                .changed()
                .await
                .map_err(|_| "Media scan session closed")?;
            completed &= !*canceled.borrow();
        }
    }

    pub fn configure(
        &self,
        policy: crate::features::services::files::media_scan_policy::Policy,
    ) -> Result<(), String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        if guard.jobs.iter().any(|job| {
            job.snapshot.volume_id.is_none()
                && matches!(
                    job.snapshot.status,
                    Status::Queued | Status::Running | Status::Paused | Status::Canceling
                )
        }) {
            return Err("Media scan already running".into());
        }
        guard.policy = Some(Arc::new(policy));
        Ok(())
    }

    pub fn policy(
        &self,
    ) -> Result<Arc<crate::features::services::files::media_scan_policy::Policy>, String> {
        self.state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .policy
            .clone()
            .ok_or_else(|| "Media scan directories not configured".into())
    }

    pub fn publish_catalog_types(&self, types: impl IntoIterator<Item = i32>) {
        if let Some(events) = &self.events {
            crate::features::services::files::catalog_events::publish(events, types);
        }
    }

    pub fn record_completion<T>(
        &self,
        record: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        let result = record();
        drop(guard);
        result
    }

    pub fn record_with_scope<T>(
        &self,
        record: impl FnOnce(Option<&crate::features::files::index_scope::Scope>) -> Result<T, String>,
    ) -> Result<T, String> {
        let guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        record(
            guard
                .policy
                .as_ref()
                .and_then(|policy| policy.scope.as_ref()),
        )
    }

    pub fn snapshot(&self) -> Result<Option<Snapshot>, String> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .rebuild_snapshot()
            .cloned())
    }

    pub fn snapshot_for(&self, volume_id: &str) -> Result<Option<Snapshot>, String> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?
            .jobs
            .iter()
            .find(|job| job.snapshot.volume_id.as_deref() == Some(volume_id))
            .map(|job| job.snapshot.clone()))
    }

    pub fn pause(&self, volume_id: Option<&str>) -> Result<Option<Snapshot>, String> {
        self.set_paused(volume_id, true)
    }

    pub fn resume(&self, volume_id: Option<&str>) -> Result<Option<Snapshot>, String> {
        self.set_paused(volume_id, false)
    }

    fn set_paused(
        &self,
        volume_id: Option<&str>,
        paused: bool,
    ) -> Result<Option<Snapshot>, String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        if guard.closed {
            return Err("Rust core stopped".into());
        }
        let mut changed = Vec::new();
        for job in guard.jobs.iter_mut().filter(|job| {
            job.purpose == Purpose::Rebuild
                && volume_id.is_none_or(|id| job.snapshot.volume_id.as_deref() == Some(id))
        }) {
            if paused && matches!(job.snapshot.status, Status::Running | Status::Queued) {
                job.snapshot.status = Status::Paused;
            } else if !paused && job.snapshot.status == Status::Paused {
                job.snapshot.status = Status::Queued;
            } else {
                continue;
            }
            changed.push(job.snapshot.id.clone());
            job.activity.send_replace(());
        }
        guard.schedule();
        for id in changed {
            self.publish(&guard, &id);
        }
        Ok(guard.rebuild_snapshot().cloned())
    }

    pub fn cancel(&self, volume_id: Option<&str>) -> Result<Option<Snapshot>, String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        for id in Self::cancel_jobs(&mut guard, volume_id) {
            self.publish(&guard, &id);
        }
        Ok(guard.rebuild_snapshot().cloned())
    }

    fn cancel_jobs(guard: &mut State, volume_id: Option<&str>) -> Vec<String> {
        let mut changed = Vec::new();
        for job in guard
            .jobs
            .iter_mut()
            .filter(|job| volume_id.is_none_or(|id| job.snapshot.volume_id.as_deref() == Some(id)))
        {
            if matches!(
                job.snapshot.status,
                Status::Running | Status::Paused | Status::Queued
            ) {
                job.snapshot.status = if job.snapshot.status == Status::Running {
                    Status::Canceling
                } else {
                    Status::Canceled
                };
                job.cancel.send_replace(true);
                job.activity.send_replace(());
                changed.push(job.snapshot.id.clone());
            }
        }
        guard.schedule();
        changed
    }

    pub fn close(&self) -> Result<(), String> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        guard.closed = true;
        for id in Self::cancel_jobs(&mut guard, None) {
            self.publish(&guard, &id);
        }
        Ok(())
    }
}

pub struct Session {
    scans: Arc<MediaScans>,
    id: String,
    pub cancel: watch::Receiver<bool>,
}

impl Session {
    pub async fn wait_running(&self) -> Result<(), String> {
        loop {
            let mut activity = {
                let guard = self
                    .scans
                    .state
                    .lock()
                    .map_err(|_| "Media scan state unavailable")?;
                if guard.closed {
                    return Err("Rust core stopped".into());
                }
                let job = guard
                    .jobs
                    .iter()
                    .find(|job| job.snapshot.id == self.id)
                    .ok_or("Media scan replaced")?;
                match job.snapshot.status {
                    Status::Running => return Ok(()),
                    Status::Paused | Status::Queued => job.activity.subscribe(),
                    _ => return Err("Media scan canceled".into()),
                }
            };
            activity
                .changed()
                .await
                .map_err(|_| "Media scan session closed")?;
        }
    }

    pub async fn record_when_running<T>(
        &self,
        file_count: usize,
        record: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let mut record = Some(record);
        loop {
            let mut activity = {
                let mut guard = self
                    .scans
                    .state
                    .lock()
                    .map_err(|_| "Media scan state unavailable")?;
                if guard.closed {
                    return Err("Rust core stopped".into());
                }
                let job = guard
                    .jobs
                    .iter_mut()
                    .find(|job| job.snapshot.id == self.id)
                    .ok_or("Media scan replaced")?;
                match job.snapshot.status {
                    Status::Running => {
                        let value = record.take().ok_or("Media scan record unavailable")?()?;
                        job.snapshot.processed_files += file_count;
                        self.scans.publish(&guard, &self.id);
                        return Ok(value);
                    }
                    Status::Paused | Status::Queued => job.activity.subscribe(),
                    _ => return Err("Media scan canceled".into()),
                }
            };
            activity
                .changed()
                .await
                .map_err(|_| "Media scan session closed")?;
        }
    }

    pub fn record<T>(&self, record: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let mut guard = self
            .scans
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        let job = guard
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == self.id)
            .ok_or("Media scan replaced")?;
        if job.snapshot.status != Status::Running {
            return Err("Media scan canceled".into());
        }
        let value = record()?;
        job.snapshot.processed_files += 1;
        self.scans.publish(&guard, &self.id);
        Ok(value)
    }

    pub fn sources(&self, paths: &[String]) -> Result<(), String> {
        let mut guard = self
            .scans
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        let job = guard
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == self.id)
            .ok_or("Media scan replaced")?;
        job.snapshot.sources = paths
            .iter()
            .map(|path| SourceCoverage {
                path: path.clone(),
                state: CoverageState::Pending,
                error: None,
            })
            .collect();
        Ok(())
    }

    pub fn source_state(
        &self,
        path: &str,
        state: CoverageState,
        error: Option<String>,
    ) -> Result<(), String> {
        let mut guard = self
            .scans
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        let job = guard
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == self.id)
            .ok_or("Media scan replaced")?;
        if let Some(source) = job
            .snapshot
            .sources
            .iter_mut()
            .find(|source| source.path == path)
        {
            source.state = state;
            source.error = error;
        }
        self.scans.publish(&guard, &self.id);
        Ok(())
    }

    pub fn discovered(&self, total_files: usize) -> Result<(), String> {
        let mut guard = self
            .scans
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        let job = guard
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == self.id)
            .ok_or("Media scan replaced")?;
        if !matches!(
            job.snapshot.status,
            Status::Running | Status::Paused | Status::Queued
        ) {
            return Err("Media scan canceled".into());
        }
        job.snapshot.total_files = Some(total_files);
        self.scans.publish(&guard, &self.id);
        Ok(())
    }

    pub fn finish(&self, result: Result<(), String>) -> Result<Snapshot, String> {
        let mut guard = self
            .scans
            .state
            .lock()
            .map_err(|_| "Media scan state unavailable")?;
        let job = guard
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id == self.id)
            .ok_or("Media scan replaced")?;
        if job.snapshot.status == Status::Canceling {
            job.snapshot.status = Status::Canceled;
        } else if matches!(job.snapshot.status, Status::Running | Status::Paused) {
            match result {
                Ok(()) => job.snapshot.status = Status::Completed,
                Err(error) => {
                    job.snapshot.status = Status::Failed;
                    job.snapshot.error = Some(error);
                }
            }
        }
        if matches!(job.snapshot.status, Status::Canceled | Status::Failed) {
            let _ = job.cancel.send(true);
        }
        job.activity.send_replace(());
        let snapshot = job.snapshot.clone();
        guard.schedule();
        self.scans.publish(&guard, &self.id);
        Ok(snapshot)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.scans.state.lock()
            && let Some(job) = guard.jobs.iter_mut().find(|job| job.snapshot.id == self.id)
            && matches!(
                job.snapshot.status,
                Status::Queued | Status::Running | Status::Paused | Status::Canceling
            )
        {
            let _ = job.cancel.send(true);
            job.activity.send_replace(());
            job.snapshot.status = Status::Canceled;
            guard.schedule();
            self.scans.publish(&guard, &self.id);
        }
    }
}

pub async fn wait_stop(receiver: &mut watch::Receiver<bool>) {
    let _ = receiver.wait_for(|stopped| *stopped).await;
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/media_scans.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/volume_queue.rs"]
mod volume_queue;
