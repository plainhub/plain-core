use crate::features::{media::duration_cache::DurationCache, recent_files::RecentFiles};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::watch, task::JoinHandle};

pub async fn flush(
    recent: Arc<RecentFiles>,
    durations: Arc<DurationCache>,
    db: Arc<crate::infrastructure::db::Db>,
    force: bool,
) {
    let results = tokio::task::spawn_blocking(move || {
        let now = Instant::now();
        [
            (
                "Session activity",
                db.flush_session_activity(now, force)
                    .map_err(anyhow::Error::from),
            ),
            (
                "Recent files",
                if force {
                    recent.flush()
                } else {
                    recent.flush_due(now)
                },
            ),
            (
                "Media durations",
                if force {
                    durations.flush()
                } else {
                    durations.flush_due(now)
                },
            ),
        ]
    })
    .await
    .expect("Buffered write task failed");
    for (name, result) in results {
        if let Err(error) = result {
            log::error!("{name} buffered write failed: {error}");
        }
    }
}

pub fn start(
    recent: Arc<RecentFiles>,
    durations: Arc<DurationCache>,
    db: Arc<crate::infrastructure::db::Db>,
    mut stop: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        while !*stop.borrow() {
            tokio::select! {
                changed = stop.changed() => { if changed.is_err() || *stop.borrow() { break; } },
                _ = interval.tick() => flush(recent.clone(), durations.clone(), db.clone(), false).await,
            }
        }
    })
}
