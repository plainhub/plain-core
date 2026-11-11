use crate::infrastructure::db::Db;
use crate::infrastructure::events::WsEvent;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tokio::sync::{Semaphore, broadcast, watch};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum FeedWorkerStatus {
    Queued,
    Running,
    Cancelled,
    Completed,
    Error,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, plain_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct FeedSyncState {
    pub feed_id: plain_graphql::ID,
    pub task_id: plain_graphql::ID,
    pub revision: crate::infrastructure::types::Long,
    pub status: FeedWorkerStatus,
    pub error: String,
}
impl FeedSyncState {
    fn active(&self) -> bool {
        matches!(
            self.status,
            FeedWorkerStatus::Queued | FeedWorkerStatus::Running
        )
    }
}
struct Task {
    state: watch::Sender<FeedSyncState>,
    cancel: watch::Sender<bool>,
    manual: std::sync::atomic::AtomicBool,
}
#[derive(Default)]
struct State {
    tasks: BTreeMap<String, Arc<Task>>,
    batch: Option<watch::Sender<FeedSyncState>>,
    stopped: bool,
    deleting: BTreeSet<String>,
}
struct Deletion<'a> {
    service: &'a SyncService,
    id: &'a str,
}
impl Drop for Deletion<'_> {
    fn drop(&mut self) {
        self.service.state.lock().unwrap().deleting.remove(self.id);
    }
}
pub struct SyncService {
    db: Arc<Db>,
    events: broadcast::Sender<WsEvent>,
    host: Arc<crate::infrastructure::host::bridge::Host>,
    state: Mutex<State>,
    assets: Option<Arc<crate::features::services::feeds::FeedAssets>>,
    permits: Semaphore,
    scheduler: Mutex<Option<tokio::task::JoinHandle<()>>>,
    revision: std::sync::atomic::AtomicI64,
    notifications: Mutex<bool>,
}
impl SyncService {
    pub fn new(
        db: Arc<Db>,
        events: broadcast::Sender<WsEvent>,
        host: Arc<crate::infrastructure::host::bridge::Host>,
        assets: Option<Arc<crate::features::services::feeds::FeedAssets>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            events,
            host,
            state: Mutex::new(State::default()),
            assets,
            permits: Semaphore::new(16),
            scheduler: Mutex::new(None),
            revision: std::sync::atomic::AtomicI64::new(0),
            notifications: Mutex::new(false),
        })
    }
    fn next_revision(&self) -> crate::infrastructure::types::Long {
        crate::infrastructure::types::Long(
            self.revision
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1,
        )
    }
    fn cancel_automatic(&self) {
        for task in self.state.lock().unwrap().tasks.values() {
            if !task.manual.load(std::sync::atomic::Ordering::SeqCst) {
                task.cancel.send_replace(true);
            }
        }
    }
    pub fn states(&self) -> Vec<FeedSyncState> {
        let state = self.state.lock().unwrap();
        state
            .tasks
            .values()
            .map(|task| task.state.borrow().clone())
            .chain(state.batch.iter().map(|batch| batch.borrow().clone()))
            .collect()
    }
    pub fn notify_snapshot(&self) -> Result<(), String> {
        let _delivery = self.notifications.lock().unwrap();
        self.host.notify(
            "feedSyncChanged",
            serde_json::json!({"states":self.states(),"removed":[],"replace":true}),
        )
    }
    fn notify(self: &Arc<Self>, ids: &[String], removed: &[String]) {
        if ids.is_empty() && removed.is_empty() {
            return;
        }
        let mut recovering = self.notifications.lock().unwrap();
        let runtime = self.state.lock().unwrap();
        let states = ids
            .iter()
            .filter_map(|id| {
                if id == "all" {
                    runtime.batch.as_ref().map(|batch| batch.borrow().clone())
                } else {
                    runtime
                        .tasks
                        .get(id)
                        .map(|task| task.state.borrow().clone())
                }
            })
            .collect::<Vec<_>>();
        drop(runtime);
        let payload = serde_json::json!({"states":states,"removed":removed,"replace":false});
        if let Err(error) = self.host.notify("feedSyncChanged", payload) {
            log::warn!("Feed sync callback: {error}");
            if *recovering || !self.host.connected() {
                return;
            }
            *recovering = true;
            let service = self.clone();
            tokio::spawn(async move {
                let permit = service.host.reserve_callback().await;
                let mut recovering = service.notifications.lock().unwrap();
                if let Ok(permit) = permit {
                    let snapshot =
                        serde_json::json!({"states":service.states(),"removed":[],"replace":true});
                    if let Err(error) =
                        service
                            .host
                            .notify_reserved_callback("feedSyncChanged", snapshot, permit)
                    {
                        log::warn!("Feed sync recovery callback: {error}");
                    }
                }
                *recovering = false;
            });
        }
    }
    pub fn queue(self: &Arc<Self>, id: Option<String>) -> plain_libs::anyhow::Result<()> {
        let feeds = if let Some(id) = id.as_ref() {
            vec![
                self.db
                    .feed_get(id)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("feed_not_found"))?,
            ]
        } else {
            self.db.feeds_list()?
        };
        self.queue_feeds(feeds, true, id.is_none())?;
        Ok(())
    }
    fn queue_feeds(
        self: &Arc<Self>,
        feeds: Vec<crate::infrastructure::db::notes_feeds::FeedRow>,
        manual: bool,
        aggregate: bool,
    ) -> plain_libs::anyhow::Result<()> {
        let mut starts = Vec::new();
        let mut subscriptions = Vec::new();
        let mut state = self.state.lock().unwrap();
        plain_libs::anyhow::ensure!(!state.stopped, "Feed sync stopped");
        let feeds = feeds
            .into_iter()
            .filter(|feed| !state.deleting.contains(&feed.id))
            .map(|feed| self.db.feed_get(&feed.id))
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        plain_libs::anyhow::ensure!(
            !feeds
                .iter()
                .any(|feed| state.tasks.get(&feed.id).is_some_and(|task| task
                    .state
                    .borrow()
                    .active()
                    && *task.cancel.borrow())),
            "Feed sync cancellation is in progress"
        );
        for feed in feeds {
            if let Some(task) = state
                .tasks
                .get(&feed.id)
                .filter(|task| task.state.borrow().active())
            {
                if manual {
                    task.manual.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if aggregate {
                    subscriptions.push(task.state.subscribe());
                }
                continue;
            }
            let (status, receiver) = watch::channel(FeedSyncState {
                feed_id: feed.id.clone().into(),
                task_id: plain_libs::utils::uuid::Uuid::new_v4().into_string().into(),
                revision: self.next_revision(),
                status: FeedWorkerStatus::Queued,
                error: String::new(),
            });
            let (cancel, _) = watch::channel(false);
            let task = Arc::new(Task {
                state: status,
                cancel,
                manual: std::sync::atomic::AtomicBool::new(manual),
            });
            state.tasks.insert(feed.id.clone(), task.clone());
            if aggregate {
                subscriptions.push(receiver);
            }
            starts.push((feed, task));
        }
        let batch = if aggregate {
            let (sender, _) = watch::channel(FeedSyncState {
                feed_id: "all".into(),
                task_id: plain_libs::utils::uuid::Uuid::new_v4().into_string().into(),
                revision: self.next_revision(),
                status: FeedWorkerStatus::Running,
                error: String::new(),
            });
            state.batch = Some(sender.clone());
            Some(sender)
        } else {
            None
        };
        drop(state);
        let mut changed = starts
            .iter()
            .map(|(feed, _)| feed.id.clone())
            .collect::<Vec<_>>();
        if aggregate {
            changed.push("all".into());
        }
        self.notify(&changed, &[]);
        for (feed, task) in starts {
            let this = self.clone();
            tokio::spawn(async move { this.run(feed, task).await });
        }
        if let Some(batch) = batch {
            let this = self.clone();
            tokio::spawn(async move {
                let mut errors = Vec::new();
                let mut cancelled = false;
                for receiver in &mut subscriptions {
                    while receiver.borrow().active() {
                        if receiver.changed().await.is_err() {
                            break;
                        }
                    }
                    let state = receiver.borrow().clone();
                    cancelled |= state.status == FeedWorkerStatus::Cancelled;
                    if !state.error.is_empty() {
                        errors.push(state.error);
                    }
                }
                let mut result = batch.borrow().clone();
                result.status = if cancelled {
                    FeedWorkerStatus::Cancelled
                } else if errors.is_empty() {
                    FeedWorkerStatus::Completed
                } else {
                    FeedWorkerStatus::Error
                };
                result.error = errors.join("\n");
                result.revision = this.next_revision();
                batch.send_replace(result.clone());
                let current = this
                    .state
                    .lock()
                    .unwrap()
                    .batch
                    .as_ref()
                    .is_some_and(|current| current.borrow().task_id == result.task_id);
                if current {
                    this.notify(&["all".into()], &[]);
                }
            });
        }
        Ok(())
    }
    async fn run(
        self: Arc<Self>,
        feed: crate::infrastructure::db::notes_feeds::FeedRow,
        task: Arc<Task>,
    ) {
        let mut cancel = task.cancel.subscribe();
        let assets = self.assets.as_ref().map(|assets| assets.scoped());
        let work = async {
            let _permit = self
                .permits
                .acquire()
                .await
                .map_err(|error| error.to_string())?;
            let mut state = task.state.borrow().clone();
            state.status = FeedWorkerStatus::Running;
            state.revision = self.next_revision();
            task.state.send_replace(state);
            self.notify(&[feed.id.clone()], &[]);
            fetch_with_retry(|| async {
                crate::features::services::feeds::sync_feed(
                    &self.db,
                    &self.events,
                    &feed,
                    assets.as_ref(),
                )
                .await
                .map_err(|error| error.to_string())
            })
            .await
        };
        let result = tokio::select! {
            biased;
            _ = async { if !*cancel.borrow() { let _ = cancel.changed().await; } } => None,
            result = work => Some(result),
        };
        if let Some(assets) = &assets {
            assets.wait_writes().await;
        }
        let mut state = task.state.borrow().clone();
        match result {
            None => {
                state.status = FeedWorkerStatus::Cancelled;
                state.error.clear();
                let _ = self.events.send(WsEvent::broadcast(
                    "FEEDS_FETCHED",
                    serde_json::json!({"feedId":feed.id,"cancelled":true}).to_string(),
                ));
            }
            Some(Ok(())) => {
                state.status = FeedWorkerStatus::Completed;
                state.error.clear();
            }
            Some(Err(error)) => {
                state.status = FeedWorkerStatus::Error;
                state.error = error;
            }
        }
        state.revision = self.next_revision();
        task.state.send_replace(state);
        self.notify(&[feed.id.clone()], &[]);
    }
    pub async fn delete(self: &Arc<Self>, id: &str) -> plain_libs::anyhow::Result<bool> {
        {
            let mut state = self.state.lock().unwrap();
            plain_libs::anyhow::ensure!(!state.stopped, "Feed sync stopped");
            plain_libs::anyhow::ensure!(
                state.deleting.insert(id.to_owned()),
                "Feed deletion is in progress"
            );
        }
        let _deletion = Deletion { service: self, id };
        self.cancel_and_wait(id).await;
        let feed = crate::features::feeds::get(&self.db, id)?;
        let entries =
            crate::features::feeds::search(&self.db, &format!("feed_id:{id}"), i32::MAX, 0)?;
        let deleted = crate::features::feeds::delete(&self.db, id)?;
        if deleted {
            self.forget(id);
            if let Some(assets) = &self.assets {
                if let Some(feed) = feed {
                    assets.release(&feed.logo);
                }
                for entry in entries {
                    assets.release(&entry.image);
                }
            }
        }
        Ok(deleted)
    }
    async fn cancel_and_wait(&self, id: &str) {
        let task = self.state.lock().unwrap().tasks.get(id).cloned();
        if let Some(task) = task {
            let mut state = task.state.subscribe();
            task.cancel.send_replace(true);
            while state.borrow().active() {
                if state.changed().await.is_err() {
                    break;
                }
            }
        }
    }
    fn forget(self: &Arc<Self>, id: &str) {
        self.state.lock().unwrap().tasks.remove(id);
        self.notify(&[], &[id.to_owned()]);
    }
    pub async fn shutdown(&self) {
        let tasks = {
            let mut state = self.state.lock().unwrap();
            state.stopped = true;
            state.tasks.values().cloned().collect::<Vec<_>>()
        };
        let scheduler = self.scheduler.lock().unwrap().take();
        if let Some(scheduler) = scheduler {
            scheduler.abort();
            if let Err(error) = scheduler.await {
                if !error.is_cancelled() {
                    log::warn!("Feed scheduler shutdown: {error}");
                }
            }
        }
        for task in &tasks {
            task.cancel.send_replace(true);
        }
        for task in tasks {
            let mut state = task.state.subscribe();
            while state.borrow().active() {
                if state.changed().await.is_err() {
                    break;
                }
            }
        }
        let batch = self
            .state
            .lock()
            .unwrap()
            .batch
            .as_ref()
            .map(watch::Sender::subscribe);
        if let Some(mut batch) = batch {
            while batch.borrow().active() {
                if batch.changed().await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn fetch_with_retry<F>(mut fetch: impl FnMut() -> F) -> Result<(), String>
where
    F: std::future::Future<Output = Result<(), String>>,
{
    for attempt in 0..3 {
        match fetch().await {
            Err(error) if attempt < 2 && schedule::retryable(&error) => {
                tokio::time::sleep(std::time::Duration::from_secs(2 << attempt)).await
            }
            result => return result,
        }
    }
    unreachable!()
}

pub mod schedule;

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/feeds/sync_service.rs"]
mod tests;
