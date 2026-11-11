use plain_libs::anyhow::{Result, bail};
use plain_libs::kv::Db;
use plain_libs::kv::LazyDb;
use plain_libs::utils::write_batch::WriteBatch;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::{collections::BTreeMap, time::Instant};

const PREFIX: &str = "media_duration:";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Duration {
    pub media_type: String,
    #[serde(rename = "media_id")]
    pub file_id: String,
    pub path: String,
    pub size_bytes: i64,
    pub modified_at: String,
    pub duration_ms: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Identity {
    pub media_type: String,
    pub file_id: String,
    pub path: String,
    pub size_bytes: i64,
    pub modified_at: String,
}

pub struct DurationCache {
    store: Arc<LazyDb>,
    writes: Mutex<WriteBatch<Duration>>,
    revision: AtomicU64,
}

impl DurationCache {
    pub fn new(data_dir: &Path) -> Self {
        Self::with_store(Arc::new(LazyDb::new(
            crate::infrastructure::data_paths::DataPaths::new(data_dir).database(),
        )))
    }

    pub fn with_store(store: Arc<LazyDb>) -> Self {
        Self {
            store,
            writes: Mutex::new(WriteBatch::default()),
            revision: AtomicU64::new(0),
        }
    }

    fn with_db<T>(&self, run: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        let db = self.store.get()?;
        run(&db)
    }

    fn merged(&self, pending: &[Duration]) -> Result<BTreeMap<String, Duration>> {
        let mut values = self.with_db(|db| {
            db.scan_prefix(PREFIX)
                .map(|entry| {
                    let (key, value) = entry?;
                    Ok((key, plain_libs::serde_json::from_str(&value)?))
                })
                .collect::<Result<BTreeMap<_, Duration>>>()
        })?;
        for value in pending {
            values.insert(key(&value.media_type, &value.file_id)?, value.clone());
        }
        Ok(values)
    }

    pub fn all(&self) -> Result<Vec<Duration>> {
        let pending = self.writes.lock().unwrap();
        Ok(self.merged(pending.pending())?.into_values().collect())
    }

    fn persist(&self, pending: &[Duration]) -> Result<()> {
        self.with_db(|db| {
            let mut values = BTreeMap::new();
            for value in pending {
                values.insert(
                    key(&value.media_type, &value.file_id)?,
                    plain_libs::serde_json::to_string(value)?,
                );
            }
            let mut batch = db.batch();
            for (key, value) in values {
                batch.insert(key, value);
            }
            db.apply_batch(batch)
        })
    }

    pub fn flush_due(&self, now: Instant) -> Result<bool> {
        self.writes
            .lock()
            .unwrap()
            .flush_due(now, |pending| self.persist(pending))
    }

    pub fn flush(&self) -> Result<bool> {
        self.writes
            .lock()
            .unwrap()
            .flush(|pending| self.persist(pending))
    }

    pub fn cancel_pending_probes(&self) {
        let _write = self.writes.lock().unwrap();
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub fn matching(&self, identities: &[Identity]) -> Result<Vec<Option<i64>>> {
        if identities.is_empty() {
            return Ok(Vec::new());
        }
        let pending = self.writes.lock().unwrap();
        self.with_db(|db| {
            identities
                .iter()
                .map(|identity| {
                    let value = match pending.pending().iter().rev().find(|value| {
                        value.media_type == identity.media_type && value.file_id == identity.file_id
                    }) {
                        Some(value) => value.clone(),
                        None => {
                            let Some(bytes) =
                                db.get(key(&identity.media_type, &identity.file_id)?)?
                            else {
                                return Ok(None);
                            };
                            plain_libs::serde_json::from_str(&bytes)?
                        }
                    };
                    let modified =
                        plain_libs::chrono::DateTime::parse_from_rfc3339(&identity.modified_at)?.timestamp();
                    let stored =
                        plain_libs::chrono::DateTime::parse_from_rfc3339(&value.modified_at)?.timestamp();
                    Ok((value.path == identity.path
                        && value.size_bytes == identity.size_bytes
                        && stored == modified)
                        .then_some(value.duration_ms)
                        .filter(|duration| *duration > 0))
                })
                .collect()
        })
    }

    pub fn save_at_revision(&self, value: Duration, revision: u64) -> Result<bool> {
        let mut pending = self.writes.lock().unwrap();
        if self.revision() != revision {
            return Ok(false);
        }
        self.save_locked(value, &mut pending)?;
        Ok(true)
    }

    pub fn save(&self, value: Duration) -> Result<()> {
        let mut pending = self.writes.lock().unwrap();
        self.save_locked(value, &mut pending)
    }

    fn save_locked(&self, mut value: Duration, pending: &mut WriteBatch<Duration>) -> Result<()> {
        key(&value.media_type, &value.file_id)?;
        if value.path.is_empty() || value.size_bytes < 0 || value.duration_ms <= 0 {
            bail!("Invalid media duration cache entry");
        }
        value.modified_at = plain_libs::chrono::DateTime::parse_from_rfc3339(&value.modified_at)?
            .with_timezone(&plain_libs::chrono::Utc)
            .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Secs, true);
        value.updated_at = plain_libs::utils::dbtime::now_iso_millis();
        let now = Instant::now();
        pending.push(value, now);
        pending.flush_due(now, |values| self.persist(values))?;
        Ok(())
    }

    pub fn remove(&self, media_type: &str, ids: &[String]) -> Result<usize> {
        let keys = ids
            .iter()
            .map(|id| key(media_type, id))
            .collect::<Result<std::collections::HashSet<_>>>()?;
        if keys.is_empty() {
            return Ok(0);
        }
        let mut pending = self.writes.lock().unwrap();
        self.revision.fetch_add(1, Ordering::AcqRel);
        let removed = self.with_db(|db| {
            let mut batch = db.batch();
            let mut changed = false;
            let mut removed = 0;
            for key in &keys {
                let stored = db.get(key)?.is_some();
                if stored {
                    batch.remove(key);
                    changed = true;
                }
                let buffered = pending
                    .pending()
                    .iter()
                    .any(|value| format!("{PREFIX}{}:{}", value.media_type, value.file_id) == *key);
                if stored || buffered {
                    removed += 1;
                }
            }
            if changed {
                db.apply_batch(batch)?;
            }
            Ok(removed)
        })?;
        pending.retain(|value| {
            !keys.contains(&format!("{PREFIX}{}:{}", value.media_type, value.file_id))
        });
        Ok(removed)
    }

    fn remove_persisted(&self, keys: &std::collections::HashSet<String>) -> Result<()> {
        self.with_db(|db| {
            let mut batch = db.batch();
            let mut changed = false;
            for key in keys {
                if db.get(key)?.is_some() {
                    batch.remove(key);
                    changed = true;
                }
            }
            if changed {
                db.apply_batch(batch)?;
            }
            Ok(())
        })
    }

    pub fn invalidate_paths(&self, paths: &[String], roots: &[String]) -> Result<()> {
        if paths.is_empty() && roots.is_empty() {
            return Ok(());
        }
        let mut pending = self.writes.lock().unwrap();
        self.revision.fetch_add(1, Ordering::AcqRel);
        let keys = self
            .merged(pending.pending())?
            .into_iter()
            .filter_map(|(key, value)| {
                let path = Path::new(&value.path);
                (paths.iter().any(|item| path == Path::new(item))
                    || roots.iter().any(|root| path.starts_with(root)))
                .then_some(key)
            })
            .collect();
        self.remove_persisted(&keys)?;
        pending.retain(|value| {
            !keys.contains(&format!("{PREFIX}{}:{}", value.media_type, value.file_id))
        });
        Ok(())
    }
}

fn key(media_type: &str, id: &str) -> Result<String> {
    if !matches!(media_type, "audio" | "video") || id.is_empty() {
        bail!("Invalid media duration identity");
    }
    Ok(format!("{PREFIX}{media_type}:{id}"))
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/media/duration_cache.rs"]
mod tests;
