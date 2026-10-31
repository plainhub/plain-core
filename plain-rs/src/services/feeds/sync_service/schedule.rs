use anyhow::{Result, ensure};
use plain_rs::platform::network::host as network;
use plain_rs::platform::network::lan::NetworkType;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::feeds::sync_service::*;
use plain_rs::{db::notes_feeds::FeedRow, prefs::settings::Settings};
use std::time::Duration;

const RESIDENT: &str = "resident";
const RETRY_SECONDS: i64 = 60;

pub(super) fn retryable(error: &str) -> bool {
    error.starts_with("TIMEOUT:")
        || error.starts_with("DNS:")
        || error
            .strip_prefix("SERVER:")
            .and_then(|detail| detail.split(':').next())
            .and_then(|status| status.parse::<u16>().ok())
            .is_some_and(|status| status == 429 || (500..600).contains(&status))
}

fn next_at(last: Option<&str>, interval: i32, now_ms: i64) -> Result<i64> {
    ensure!(interval > 0, "Invalid feed refresh interval");
    let Some(last) = last else {
        return Ok(now_ms);
    };
    let last = chrono::DateTime::parse_from_rfc3339(last)?.timestamp_millis();
    if last > now_ms {
        return Ok(now_ms);
    }
    Ok(last.saturating_add(i64::from(interval) * 1000))
}
fn due(
    settings: &Settings,
    feeds: Vec<FeedRow>,
    now_ms: i64,
) -> Result<(Vec<FeedRow>, Option<i64>)> {
    if !settings.feed_auto_refresh {
        return Ok((Vec::new(), None));
    }
    let mut ready = Vec::new();
    let mut earliest = None;
    for feed in feeds {
        let at = next_at(
            feed.last_sync_at.as_deref(),
            settings.feed_auto_refresh_interval,
            now_ms,
        )?;
        earliest = Some(earliest.map_or(at, |current: i64| current.min(at)));
        if at <= now_ms {
            ready.push(feed);
        }
    }
    Ok((ready, earliest))
}
async fn scheduled_feeds(
    db: Arc<Db>,
    settings: &Settings,
    now: i64,
) -> Result<(Vec<FeedRow>, Option<i64>)> {
    let settings = settings.clone();
    tokio::task::spawn_blocking(move || due(&settings, db.feeds_list()?, now)).await?
}

fn allowed(only_wifi: bool, network: NetworkType) -> bool {
    network != NetworkType::None && (!only_wifi || network == NetworkType::Wifi)
}
async fn queue_due(
    state: &CoreRuntime,
    lease: &str,
) -> Result<Vec<watch::Receiver<FeedSyncState>>> {
    let _policy = state.feeds.policy_gate.lock().await;
    let settings = plain_rs::prefs::settings::snapshot(&state.prefs)?;
    if !settings.feed_auto_refresh {
        state.feeds.apply_auto_policy(false, false);
    }
    let (feeds, _) = scheduled_feeds(
        state.db.clone(),
        &settings,
        chrono::Utc::now().timestamp_millis(),
    )
    .await?;
    if feeds.is_empty() {
        return Ok(Vec::new());
    }
    let capabilities = plain_rs::services::capabilities::snapshot(&state.host).await?;
    plain_rs::services::capabilities::require_list(&capabilities, "FEEDS")?;
    plain_rs::services::capabilities::require_list(&capabilities, "BACKGROUND_WAKE")?;
    let permitted = allowed(
        settings.feed_auto_refresh_only_wifi,
        network::network_type(&state.host)
            .await
            .map_err(anyhow::Error::msg)?,
    );
    state.feeds.apply_auto_policy(permitted, !permitted);
    if !permitted {
        return Ok(Vec::new());
    }
    state.feeds.queue_feeds(feeds, Some(lease), false)
}

#[derive(Clone)]
pub(super) struct Lease {
    pub(super) revoke: watch::Sender<bool>,
    pub(super) done: watch::Sender<bool>,
}

pub fn grant(state: &CoreRuntime, id: String, deadline_ms: i64, generation: u64) -> Result<()> {
    ensure!(
        !id.is_empty() && id.len() <= 128 && id != RESIDENT,
        "Invalid background execution ID"
    );
    ensure!(
        deadline_ms > chrono::Utc::now().timestamp_millis(),
        "Background execution already expired"
    );
    let lease = register(&state.feeds, &id)?;
    let remaining = Duration::from_millis(
        deadline_ms
            .saturating_sub(chrono::Utc::now().timestamp_millis())
            .max(0) as u64,
    );
    let deadline = tokio::time::Instant::now() + remaining;
    let state = state.clone();
    tokio::spawn(async move {
        let work = async {
            let mut tasks = queue_due(&state, &id).await?;
            for task in &mut tasks {
                while task.borrow().active() {
                    task.changed().await?;
                }
            }
            Ok(tasks.iter().all(|task| task.borrow().status == "COMPLETED"))
        };
        let success = execute_lease(state.feeds.clone(), &id, deadline, lease, work).await;
        if let Err(error) = state
            .host
            .call_current(
                generation,
                "systemCompleteBackgroundExecution",
                serde_json::json!({"id":id,"success":success}),
            )
            .await
        {
            log::warn!("Background execution completion failed: {error}");
        }
    });
    Ok(())
}

fn register(service: &SyncService, id: &str) -> Result<Lease> {
    let runtime = service.state.lock().unwrap();
    ensure!(!runtime.stopped, "Feed sync stopped");
    let mut leases = service.leases.lock().unwrap();
    ensure!(
        !leases.contains_key(id),
        "Background execution ID already active"
    );
    ensure!(
        leases.len() < plain_rs::host::bridge::REQUEST_CAPACITY,
        "Background execution capacity exceeded"
    );
    let (revoke, _) = watch::channel(false);
    let (done, _) = watch::channel(false);
    let lease = Lease { revoke, done };
    leases.insert(id.to_owned(), lease.clone());
    Ok(lease)
}

async fn execute_lease(
    service: Arc<SyncService>,
    id: &str,
    deadline: tokio::time::Instant,
    lease: Lease,
    work: impl std::future::Future<Output = Result<bool>>,
) -> bool {
    let mut revoked = lease.revoke.subscribe();
    let success = tokio::select! {
        biased;
        _ = async { if !*revoked.borrow() { let _ = revoked.changed().await; } } => false,
        _ = tokio::time::sleep_until(deadline) => false,
        result = work => match result {
            Ok(success) => success,
            Err(error) => { log::warn!("Background execution {id} failed: {error}"); false }
        },
    };
    let tasks = service
        .state
        .lock()
        .unwrap()
        .tasks
        .values()
        .filter(|task| task.owners.lock().unwrap().leases.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    service.release_lease(id);
    for task in tasks {
        if !*task.cancel.borrow() {
            continue;
        }
        let mut status = task.state.subscribe();
        while status.borrow().active() {
            if status.changed().await.is_err() {
                break;
            }
        }
    }
    service.leases.lock().unwrap().remove(id);
    lease.done.send_replace(true);
    success
}

pub async fn revoke(state: &CoreRuntime, id: &str) {
    let lease = state.feeds.leases.lock().unwrap().get(id).cloned();
    if let Some(lease) = lease {
        let mut done = lease.done.subscribe();
        lease.revoke.send_replace(true);
        while !*done.borrow() {
            if done.changed().await.is_err() {
                break;
            }
        }
    }
}

pub fn start(state: CoreRuntime) {
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let mut events = state.events.subscribe();
        let mut network = state.network_revision.subscribe();
        let mut connection = state.host.connection_changes();
        let mut wake = None;
        loop {
            let mut delay = Duration::from_secs(RETRY_SECONDS as u64);
            if *stop.borrow() {
                break;
            }
            if state.host.connected() {
                let result = tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    _ = connection.changed() => {
                        state.feeds.release_lease(RESIDENT);
                        for lease in state.feeds.leases.lock().unwrap().values() { lease.revoke.send_replace(true); }
                        wake = None;
                        continue;
                    },
                    result = tick(&state, &mut wake) => result,
                };
                match result {
                    Ok(next) => delay = next,
                    Err(error) => log::warn!("Feed scheduler: {error}"),
                }
            } else {
                state.feeds.release_lease(RESIDENT);
                for lease in state.feeds.leases.lock().unwrap().values() {
                    lease.revoke.send_replace(true);
                }
                wake = None;
            }
            tokio::select! {
                _ = stop.changed() => break,
                _ = connection.changed() => {},
                _ = network.changed() => {},
                _ = state.feeds.execution_changed.notified() => {},
                _ = tokio::time::sleep(delay) => {},
                _ = async {
                    loop {
                        match events.recv().await {
                            Ok(event) if matches!(event.event_type, "PREFS_UPDATED" | "FEEDS_FETCHED" | "CONTENT_CHANGED") => break,
                            Err(_) => break,
                            _ => {},
                        }
                    }
                } => {},
            }
        }
        state.feeds.release_lease(RESIDENT);
        for lease in state.feeds.leases.lock().unwrap().values() {
            lease.revoke.send_replace(true);
        }
    });
}
async fn tick(state: &CoreRuntime, previous_wake: &mut Option<Option<i64>>) -> Result<Duration> {
    let _policy = state.feeds.policy_gate.lock().await;
    let settings = plain_rs::prefs::settings::snapshot(&state.prefs)?;
    if !settings.feed_auto_refresh {
        state.feeds.apply_auto_policy(false, false);
    }
    let now = chrono::Utc::now().timestamp_millis();
    let (feeds, earliest) = scheduled_feeds(state.db.clone(), &settings, now).await?;
    let active_auto = state
        .feeds
        .state
        .lock()
        .unwrap()
        .tasks
        .values()
        .any(|task| task.state.borrow().active() && !task.owners.lock().unwrap().manual);
    if earliest.is_none() && previous_wake.is_none() && !active_auto {
        return Ok(Duration::from_secs(RETRY_SECONDS as u64));
    }
    let capabilities = plain_rs::services::capabilities::snapshot(&state.host).await?;
    let supported = |name: &str| capabilities.iter().any(|capability| capability == name);
    if !supported("BACKGROUND_TIMER") {
        state.feeds.release_lease(RESIDENT);
    }
    if !supported("FEEDS") {
        state.feeds.apply_auto_policy(false, true);
    } else if settings.feed_auto_refresh && (!feeds.is_empty() || active_auto) {
        let permitted = allowed(
            settings.feed_auto_refresh_only_wifi,
            network::network_type(&state.host)
                .await
                .map_err(anyhow::Error::msg)?,
        );
        state.feeds.apply_auto_policy(permitted, !permitted);
        if permitted && supported("BACKGROUND_TIMER") && !feeds.is_empty() {
            state.feeds.queue_feeds(feeds, Some(RESIDENT), false)?;
        }
    }
    let wake = if supported("FEEDS") {
        earliest.map(|at| {
            if at > now {
                return at;
            }
            previous_wake
                .flatten()
                .filter(|previous| *previous > now)
                .unwrap_or(now.saturating_add(RETRY_SECONDS * 1000))
        })
    } else {
        None
    };
    if supported("BACKGROUND_WAKE") && previous_wake.as_ref() != Some(&wake) {
        state
            .host
            .call(
                "systemScheduleBackgroundWake",
                serde_json::json!({"atMs":wake}),
            )
            .await
            .map_err(anyhow::Error::msg)?;
        *previous_wake = Some(wake);
    }
    let delay = earliest
        .filter(|at| *at > now)
        .map(|at| at.saturating_sub(now).min(RETRY_SECONDS * 1000))
        .unwrap_or(RETRY_SECONDS * 1000);
    Ok(Duration::from_millis(delay as u64))
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/feeds/sync_service/schedule.rs"]
mod tests;
