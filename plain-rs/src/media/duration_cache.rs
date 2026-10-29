use crate::kv::{Db, LazyDb};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

const PREFIX: &str = "android_media_duration:";

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
    writes: Mutex<()>,
    revision: AtomicU64,
}

impl DurationCache {
    pub fn new(data_dir: &Path) -> Self {
        Self::with_store(Arc::new(LazyDb::new(
            crate::data_paths::DataPaths::new(data_dir).database(),
        )))
    }

    pub fn with_store(store: Arc<LazyDb>) -> Self {
        Self {
            store,
            writes: Mutex::new(()),
            revision: AtomicU64::new(0),
        }
    }

    fn with_db<T>(&self, run: impl FnOnce(&Db) -> Result<T>) -> Result<T> {
        let db = self.store.get()?;
        run(&db)
    }

    pub fn all(&self) -> Result<Vec<Duration>> {
        self.with_db(|db| {
            db.scan_prefix(PREFIX)
                .map(|entry| {
                    let (_, value) = entry?;
                    Ok(serde_json::from_str(&value)?)
                })
                .collect()
        })
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
        self.with_db(|db| {
            identities
                .iter()
                .map(|identity| {
                    let Some(bytes) = db.get(key(&identity.media_type, &identity.file_id)?)? else {
                        return Ok(None);
                    };
                    let value: Duration = serde_json::from_str(&bytes)?;
                    let modified =
                        chrono::DateTime::parse_from_rfc3339(&identity.modified_at)?.timestamp();
                    let stored =
                        chrono::DateTime::parse_from_rfc3339(&value.modified_at)?.timestamp();
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
        let _write = self.writes.lock().unwrap();
        if self.revision() != revision {
            return Ok(false);
        }
        self.save_locked(value)?;
        Ok(true)
    }

    pub fn save(&self, value: Duration) -> Result<()> {
        let _write = self.writes.lock().unwrap();
        self.save_locked(value)
    }

    fn save_locked(&self, mut value: Duration) -> Result<()> {
        let key = key(&value.media_type, &value.file_id)?;
        if value.path.is_empty() || value.size_bytes < 0 || value.duration_ms <= 0 {
            bail!("Invalid media duration cache entry");
        }
        value.modified_at = chrono::DateTime::parse_from_rfc3339(&value.modified_at)?
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        value.updated_at = crate::utils::dbtime::now_iso_millis();
        self.with_db(|db| db.insert(key, serde_json::to_string(&value)?))
    }

    pub fn remove(&self, media_type: &str, ids: &[String]) -> Result<usize> {
        let keys = ids
            .iter()
            .map(|id| key(media_type, id))
            .collect::<Result<Vec<_>>>()?;
        if ids.is_empty() {
            return Ok(0);
        }
        let _write = self.writes.lock().unwrap();
        self.revision.fetch_add(1, Ordering::AcqRel);
        self.with_db(|db| {
            let mut batch = db.batch();
            let mut removed = std::collections::HashSet::new();
            for key in keys {
                if db.get(&key)?.is_some() && removed.insert(key.clone()) {
                    batch.remove(key);
                }
            }
            if removed.is_empty() {
                return Ok(0);
            }
            db.apply_batch(batch)?;
            Ok(removed.len())
        })
    }

    pub fn invalidate_paths(&self, paths: &[String], roots: &[String]) -> Result<()> {
        if paths.is_empty() && roots.is_empty() {
            return Ok(());
        }
        let _write = self.writes.lock().unwrap();
        self.revision.fetch_add(1, Ordering::AcqRel);
        self.with_db(|db| {
            let mut batch = db.batch();
            let mut changed = false;
            for entry in db.scan_prefix(PREFIX) {
                let (key, value) = entry?;
                let value: Duration = serde_json::from_str(&value)?;
                let path = Path::new(&value.path);
                if paths.iter().any(|item| path == Path::new(item))
                    || roots.iter().any(|root| path.starts_with(root))
                {
                    batch.remove(key);
                    changed = true;
                }
            }
            if !changed {
                return Ok(());
            }
            db.apply_batch(batch)
        })
    }
}

fn key(media_type: &str, id: &str) -> Result<String> {
    if !matches!(media_type, "audio" | "video") || id.is_empty() {
        bail!("Invalid media duration identity");
    }
    Ok(format!("{PREFIX}{media_type}:{id}"))
}

#[cfg(test)]
#[path = "../../tests/unit/media/duration_cache.rs"]
mod tests;
