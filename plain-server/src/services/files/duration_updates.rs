use crate::host::{bridge::Host, query::Plan};
use plain_rs::{
    enums::DataType,
    media::duration_cache::{Duration, DurationCache, Identity},
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tokio::sync::{Notify, Semaphore, watch};

pub(crate) struct Runtime {
    cache: Arc<DurationCache>,
    host: Arc<Host>,
    resources: Arc<crate::host::resources::NativeResources>,
    directory: std::path::PathBuf,
    pending: Mutex<HashSet<Identity>>,
    permits: Semaphore,
    stop: watch::Sender<bool>,
    idle: Notify,
    provider_catalog: bool,
}

#[derive(Eq, PartialEq)]
struct Physical {
    size: u64,
    modified: SystemTime,
}
impl Physical {
    fn read(path: &str) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            size: metadata.len(),
            modified: metadata.modified()?,
        })
    }
    fn matches(&self, item: &Identity) -> bool {
        i64::try_from(self.size).ok() == Some(item.size_bytes)
            && chrono::DateTime::<chrono::Utc>::from(self.modified).timestamp()
                == chrono::DateTime::parse_from_rfc3339(&item.modified_at)
                    .map(|time| time.timestamp())
                    .unwrap_or(i64::MIN)
    }
}
fn identity(kind: DataType, row: &Value) -> Option<Identity> {
    let kind = match kind {
        DataType::Audio => "audio",
        DataType::Video => "video",
        _ => return None,
    };
    let modified_at = chrono::DateTime::parse_from_rfc3339(row["updatedAt"].as_str()?)
        .ok()?
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let item = Identity {
        media_type: kind.into(),
        file_id: row["id"].as_str()?.into(),
        path: row["path"].as_str()?.into(),
        size_bytes: row["size"].as_i64()?,
        modified_at,
    };
    (!item.file_id.is_empty() && Path::new(&item.path).is_absolute() && item.size_bytes >= 0)
        .then_some(item)
}
impl Runtime {
    pub(crate) fn new(
        cache: Arc<DurationCache>,
        host: Arc<Host>,
        resources: Arc<crate::host::resources::NativeResources>,
        directory: std::path::PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            cache,
            provider_catalog: host.uses_system_media_catalog(),
            host,
            resources,
            directory,
            pending: Mutex::new(HashSet::new()),
            permits: Semaphore::new(10),
            stop: watch::channel(false).0,
            idle: Notify::new(),
        })
    }
    pub(crate) async fn probe_milliseconds(&self, path: &str) -> Result<Option<i64>, String> {
        let mut core_stop = self.stop.subscribe();
        if *core_stop.borrow() {
            return Err("Media duration runtime stopped".into());
        }
        tokio::select! {
            biased;
            _ = core_stop.changed() => Err("Media duration runtime stopped".into()),
            result = super::input::read_source(&self.resources, &self.directory, path, |reader| {
                Ok(plain_rs::media::probe::probe_duration_reader(reader))
            }) => result,
        }
    }

    pub(crate) async fn enrich(
        self: &Arc<Self>,
        kind: DataType,
        rows: &mut [Value],
    ) -> Result<(), String> {
        let missing = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (row["durationMs"].as_i64().unwrap_or(0) <= 0)
                    .then(|| identity(kind, row))
                    .flatten()
                    .map(|item| (index, item))
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        let revision = self.cache.revision();
        let identities = missing
            .iter()
            .map(|(_, item)| item.clone())
            .collect::<Vec<_>>();
        let cache = self.cache.clone();
        let durations = tokio::task::spawn_blocking(move || cache.matching(&identities))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        for ((index, item), cached) in missing.into_iter().zip(durations) {
            if let Some(duration) = cached {
                rows[index]["durationMs"] = json!(duration);
            } else {
                self.enqueue(kind, item, revision);
            }
        }
        Ok(())
    }
    fn enqueue(self: &Arc<Self>, kind: DataType, item: Identity, revision: u64) {
        if *self.stop.borrow() || !self.pending.lock().unwrap().insert(item.clone()) {
            return;
        }
        let runtime = self.clone();
        let mut stop = self.stop.subscribe();
        tokio::spawn(async move {
            let stopped = *stop.borrow();
            if !stopped {
                tokio::select! {
                    biased;
                    _ = stop.changed() => {},
                    result = async {
                        let _permit = runtime.permits.acquire().await.map_err(|error| error.to_string())?;
                        if runtime.cache.revision() != revision { return Ok(()) }
                        runtime.complete(kind, item.clone(), revision).await
                    } => if let Err(error) = result { log::debug!("Media duration probe failed: {error}"); },
                }
            }
            runtime.pending.lock().unwrap().remove(&item);
            runtime.idle.notify_waiters();
        });
    }
    async fn complete(&self, kind: DataType, item: Identity, revision: u64) -> Result<(), String> {
        let input = item.clone();
        let stop = self.stop.subscribe();
        let (before, duration) = tokio::task::spawn_blocking(move || {
            let before = Physical::read(&input.path).ok()?;
            if !before.matches(&input) {
                return None;
            }
            let duration = std::fs::File::open(&input.path).ok().and_then(|file| {
                let mut reader = super::input::CancellableReader { reader: file, stop };
                plain_rs::media::probe::probe_duration_reader(&mut reader)
                    .filter(|duration| *duration > 0)
            });
            Some((before, duration))
        })
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Media identity changed before probe".to_owned())?;
        let Some(duration) = duration.filter(|duration| *duration > 0) else {
            return Ok(());
        };
        if self.provider_catalog {
            let mut plan = Plan::default();
            plan.ids("_id", vec![item.file_id.clone()]);
            let rows = self
                .host
                .call(
                    "systemMediaRows",
                    json!({
                        "dataType":kind,"plan":plan,"sortBy":"NAME_ASC","limit":1,"offset":0
                    }),
                )
                .await?;
            let current = rows.as_array().and_then(|rows| rows.first());
            if current.and_then(|row| identity(kind, row)).as_ref() != Some(&item) {
                return Ok(());
            }
            if current
                .and_then(|row| row["durationMs"].as_i64())
                .is_some_and(|duration| duration > 0)
            {
                return Ok(());
            }
        }
        let input = item.path.clone();
        let after = tokio::task::spawn_blocking(move || Physical::read(&input))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        if before != after || self.cache.revision() != revision {
            return Ok(());
        }
        let cache = self.cache.clone();
        tokio::task::spawn_blocking(move || {
            cache.save_at_revision(
                Duration {
                    media_type: item.media_type,
                    file_id: item.file_id,
                    path: item.path,
                    size_bytes: item.size_bytes,
                    modified_at: item.modified_at,
                    duration_ms: duration,
                    updated_at: String::new(),
                },
                revision,
            )
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        Ok(())
    }
    pub(crate) async fn shutdown(&self) {
        self.stop.send_replace(true);
        let cache = self.cache.clone();
        tokio::task::spawn_blocking(move || cache.cancel_pending_probes())
            .await
            .expect("Media duration cancellation failed");
        loop {
            let completed = self.idle.notified();
            tokio::pin!(completed);
            completed.as_mut().enable();
            let pending = !self.pending.lock().unwrap().is_empty();
            if !pending {
                break;
            }
            completed.await;
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/duration_updates.rs"]
mod tests;
