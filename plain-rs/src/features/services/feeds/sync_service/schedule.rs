use crate::features::prefs::settings::Settings;
use crate::features::services::feeds::sync_service::*;
use crate::infrastructure::db::notes_feeds::FeedRow;
use crate::infrastructure::platform::network::host as network;
use crate::infrastructure::platform::network::lan::NetworkType;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use std::time::Duration;

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
    let last = plain_libs::chrono::DateTime::parse_from_rfc3339(last)?.timestamp_millis();
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
pub fn start(state: CoreRuntime) {
    let service = state.feeds.clone();
    let scheduler = tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let mut events = state.events.subscribe();
        let mut network = state.network_revision.subscribe();
        let mut connection = state.host.connection_changes();
        while !*stop.borrow() {
            let mut delay = Duration::from_secs(RETRY_SECONDS as u64);
            if state.host.connected() {
                let result = tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    _ = connection.changed() => {
                        state.feeds.cancel_automatic();
                        continue;
                    },
                    result = tick(&state) => result,
                };
                match result {
                    Ok(next) => delay = next,
                    Err(error) => log::warn!("Feed scheduler: {error}"),
                }
            } else {
                state.feeds.cancel_automatic();
            }
            tokio::select! {
                _ = stop.changed() => break,
                _ = connection.changed() => {},
                _ = network.changed() => {},
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
        state.feeds.cancel_automatic();
    });
    *service.scheduler.lock().unwrap() = Some(scheduler);
}

async fn tick(state: &CoreRuntime) -> Result<Duration> {
    let settings = crate::features::prefs::settings::snapshot(&state.prefs)?;
    if !settings.feed_auto_refresh {
        state.feeds.cancel_automatic();
        return Ok(Duration::from_secs(RETRY_SECONDS as u64));
    }
    let now = plain_libs::chrono::Utc::now().timestamp_millis();
    let (feeds, earliest) = scheduled_feeds(state.db.clone(), &settings, now).await?;
    let permitted = allowed(
        settings.feed_auto_refresh_only_wifi,
        network::network_type(&state.host)
            .await
            .map_err(plain_libs::anyhow::Error::msg)?,
    );
    if permitted {
        if !feeds.is_empty() {
            state.feeds.queue_feeds(feeds, false, false)?;
        }
    } else {
        state.feeds.cancel_automatic();
    }
    let delay = earliest
        .filter(|at| *at > now)
        .map(|at| at.saturating_sub(now).min(RETRY_SECONDS * 1000))
        .unwrap_or(RETRY_SECONDS * 1000);
    Ok(Duration::from_millis(delay as u64))
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/feeds/sync_service/schedule.rs"]
mod tests;
