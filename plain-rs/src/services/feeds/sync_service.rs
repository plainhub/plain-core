use plain_rs::{db::Db, events::WsEvent};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tokio::sync::{Semaphore, broadcast, watch};

#[derive(Clone, serde::Serialize, serde::Deserialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct FeedSyncState {
    pub feed_id: async_graphql::ID,
    pub task_id: async_graphql::ID,
    pub revision: plain_rs::types::Long,
    pub status: String,
    pub error: String,
}
impl FeedSyncState {
    fn active(&self) -> bool {
        matches!(self.status.as_str(), "QUEUED" | "RUNNING")
    }
}
#[derive(Default)]
struct Owners {
    manual: bool,
    leases: BTreeSet<String>,
}
struct Task {
    state: watch::Sender<FeedSyncState>,
    cancel: watch::Sender<bool>,
    owners: Mutex<Owners>,
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
    host: Arc<plain_rs::host::bridge::Host>,
    state: Mutex<State>,
    assets: Option<Arc<plain_rs::services::feeds::FeedAssets>>,
    permits: Semaphore,
    auto_allowed: std::sync::atomic::AtomicBool,
    policy_gate: tokio::sync::Mutex<()>,
    revision: std::sync::atomic::AtomicI64,
    notifications: Mutex<bool>,
    leases: Mutex<BTreeMap<String, schedule::Lease>>,
    execution_changed: tokio::sync::Notify,
}
impl SyncService {
    pub fn new(
        db: Arc<Db>,
        events: broadcast::Sender<WsEvent>,
        host: Arc<plain_rs::host::bridge::Host>,
        assets: Option<Arc<plain_rs::services::feeds::FeedAssets>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            events,
            host,
            state: Mutex::new(State::default()),
            assets,
            permits: Semaphore::new(16),
            auto_allowed: std::sync::atomic::AtomicBool::new(false),
            policy_gate: tokio::sync::Mutex::new(()),
            revision: std::sync::atomic::AtomicI64::new(0),
            notifications: Mutex::new(false),
            leases: Mutex::new(BTreeMap::new()),
            execution_changed: tokio::sync::Notify::new(),
        })
    }
    pub fn execution_state_changed(&self) {
        self.execution_changed.notify_one();
    }

    fn next_revision(&self) -> plain_rs::types::Long {
        plain_rs::types::Long(
            self.revision
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1,
        )
    }
    fn apply_auto_policy(&self, allowed: bool, cancel_running: bool) {
        self.auto_allowed
            .store(allowed, std::sync::atomic::Ordering::SeqCst);
        if !allowed {
            for task in self.state.lock().unwrap().tasks.values() {
                if !task.owners.lock().unwrap().manual
                    && (cancel_running || task.state.borrow().status == "QUEUED")
                {
                    task.cancel.send_replace(true);
                }
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
    pub fn queue(self: &Arc<Self>, id: Option<String>) -> anyhow::Result<()> {
        let feeds = if let Some(id) = id.as_ref() {
            vec![
                self.db
                    .feed_get(id)?
                    .ok_or_else(|| anyhow::anyhow!("feed_not_found"))?,
            ]
        } else {
            self.db.feeds_list()?
        };
        self.queue_feeds(feeds, None, id.is_none())?;
        Ok(())
    }
    fn queue_feeds(
        self: &Arc<Self>,
        feeds: Vec<plain_rs::db::notes_feeds::FeedRow>,
        lease: Option<&str>,
        aggregate: bool,
    ) -> anyhow::Result<Vec<watch::Receiver<FeedSyncState>>> {
        let mut starts = Vec::new();
        let mut subscriptions = Vec::new();
        let mut state = self.state.lock().unwrap();
        anyhow::ensure!(!state.stopped, "Feed sync stopped");
        let feeds = feeds
            .into_iter()
            .filter(|feed| !state.deleting.contains(&feed.id))
            .map(|feed| self.db.feed_get(&feed.id))
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        anyhow::ensure!(
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
                let mut owners = task.owners.lock().unwrap();
                if let Some(lease) = lease {
                    owners.leases.insert(lease.to_owned());
                } else {
                    owners.manual = true;
                }
                subscriptions.push(task.state.subscribe());
                continue;
            }
            let (status, receiver) = watch::channel(FeedSyncState {
                feed_id: feed.id.clone().into(),
                task_id: uuid::Uuid::new_v4().to_string().into(),
                revision: self.next_revision(),
                status: "QUEUED".into(),
                error: String::new(),
            });
            let (cancel, _) = watch::channel(false);
            let task = Arc::new(Task {
                state: status,
                cancel,
                owners: Mutex::new(Owners {
                    manual: lease.is_none(),
                    leases: lease.into_iter().map(str::to_owned).collect(),
                }),
            });
            state.tasks.insert(feed.id.clone(), task.clone());
            subscriptions.push(receiver);
            starts.push((feed, task));
        }
        let batch = if aggregate {
            let (sender, _) = watch::channel(FeedSyncState {
                feed_id: "all".into(),
                task_id: uuid::Uuid::new_v4().to_string().into(),
                revision: self.next_revision(),
                status: "RUNNING".into(),
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
        let waiters = subscriptions.clone();
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
                    cancelled |= state.status == "CANCELLED";
                    if !state.error.is_empty() {
                        errors.push(state.error);
                    }
                }
                let mut result = batch.borrow().clone();
                result.status = if cancelled {
                    "CANCELLED"
                } else if errors.is_empty() {
                    "COMPLETED"
                } else {
                    "ERROR"
                }
                .into();
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
        Ok(waiters)
    }
    async fn run(self: Arc<Self>, feed: plain_rs::db::notes_feeds::FeedRow, task: Arc<Task>) {
        let mut cancel = task.cancel.subscribe();
        let assets = self.assets.as_ref().map(|assets| assets.scoped());
        let work = async {
            let _permit = match self.permits.acquire().await {
                Ok(permit) => permit,
                Err(error) => return Some(Err(error.to_string())),
            };
            if !task.owners.lock().unwrap().manual
                && !self.auto_allowed.load(std::sync::atomic::Ordering::SeqCst)
            {
                return None;
            }
            let mut state = task.state.borrow().clone();
            state.status = "RUNNING".into();
            state.revision = self.next_revision();
            task.state.send_replace(state);
            self.notify(&[feed.id.clone()], &[]);
            fetch_with_retry(
                || async {
                    plain_rs::services::feeds::sync_feed(
                        &self.db,
                        &self.events,
                        &feed,
                        assets.as_ref(),
                    )
                    .await
                    .map_err(|error| error.to_string())
                },
                || {
                    task.owners.lock().unwrap().manual
                        || self.auto_allowed.load(std::sync::atomic::Ordering::SeqCst)
                },
            )
            .await
        };
        let result = tokio::select! {
            biased;
            _ = async { if !*cancel.borrow() { let _ = cancel.changed().await; } } => None,
            result = work => result,
        };
        if let Some(assets) = &assets {
            assets.wait_writes().await;
        }
        let mut state = task.state.borrow().clone();
        match result {
            None => {
                state.status = "CANCELLED".into();
                state.error.clear();
                let _ = self.events.send(WsEvent::broadcast(
                    "FEEDS_FETCHED",
                    serde_json::json!({"feedId":feed.id,"cancelled":true}).to_string(),
                ));
            }
            Some(Ok(())) => {
                state.status = "COMPLETED".into();
                state.error.clear();
            }
            Some(Err(error)) => {
                state.status = "ERROR".into();
                state.error = error;
            }
        }
        state.revision = self.next_revision();
        task.state.send_replace(state);
        self.notify(&[feed.id.clone()], &[]);
    }
    fn release_lease(&self, lease: &str) {
        for task in self.state.lock().unwrap().tasks.values() {
            let mut owners = task.owners.lock().unwrap();
            owners.leases.remove(lease);
            if !owners.manual && owners.leases.is_empty() {
                task.cancel.send_replace(true);
            }
        }
    }
    pub async fn delete(self: &Arc<Self>, id: &str) -> anyhow::Result<bool> {
        {
            let mut state = self.state.lock().unwrap();
            anyhow::ensure!(!state.stopped, "Feed sync stopped");
            anyhow::ensure!(
                state.deleting.insert(id.to_owned()),
                "Feed deletion is in progress"
            );
        }
        let _deletion = Deletion { service: self, id };
        self.cancel_and_wait(id).await;
        let feed = plain_rs::feeds::get(&self.db, id)?;
        let entries = plain_rs::feeds::search(&self.db, &format!("feed_id:{id}"), i32::MAX, 0)?;
        let deleted = plain_rs::feeds::delete(&self.db, id)?;
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
        let (tasks, leases) = {
            let mut state = self.state.lock().unwrap();
            state.stopped = true;
            let leases = self
                .leases
                .lock()
                .unwrap()
                .values()
                .cloned()
                .collect::<Vec<_>>();
            (state.tasks.values().cloned().collect::<Vec<_>>(), leases)
        };
        for lease in &leases {
            lease.revoke.send_replace(true);
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
        for lease in leases {
            let mut done = lease.done.subscribe();
            while !*done.borrow() {
                if done.changed().await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn fetch_with_retry<F>(
    mut fetch: impl FnMut() -> F,
    permitted: impl Fn() -> bool,
) -> Option<Result<(), String>>
where
    F: std::future::Future<Output = Result<(), String>>,
{
    for attempt in 0..3 {
        if !permitted() {
            return None;
        }
        match fetch().await {
            Err(error) if attempt < 2 && schedule::retryable(&error) => {
                tokio::time::sleep(std::time::Duration::from_secs(2 << attempt)).await
            }
            result => return Some(result),
        }
    }
    unreachable!()
}

pub mod schedule;

#[cfg(test)]
#[path = "../../../tests/unit/services/feeds/sync_service.rs"]
mod tests;
