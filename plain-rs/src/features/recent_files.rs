use anyhow::Result;
use plain_libs::{kv::Db, utils::write_batch::WriteBatch};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Instant,
};

const KEY: &str = "recent_files";

pub struct RecentFiles {
    store: Arc<Db>,
    pending: Mutex<WriteBatch<String>>,
    writer: Mutex<()>,
    pub(crate) flush_requested: tokio::sync::Notify,
}

fn merge(pending: &[String], stored: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    pending
        .iter()
        .rev()
        .chain(stored.iter())
        .filter(|path| seen.insert(path.as_str()))
        .take(500)
        .cloned()
        .collect()
}

impl RecentFiles {
    pub fn new(store: Arc<Db>) -> Self {
        Self {
            store,
            pending: Mutex::new(WriteBatch::default()),
            writer: Mutex::new(()),
            flush_requested: tokio::sync::Notify::new(),
        }
    }

    pub fn add(&self, path: &str) {
        let now = Instant::now();
        let mut batch = self.pending.lock().unwrap();
        batch.push(path.to_owned(), now);
        if batch.is_due(now) {
            self.flush_requested.notify_one();
        }
    }

    fn persist(&self, pending: &[String]) -> Result<()> {
        self.store.update_json(KEY, |stored: &mut Vec<String>| {
            *stored = merge(pending, std::mem::take(stored));
            Ok(())
        })
    }

    pub fn list(&self, limit: usize) -> Result<Vec<String>> {
        let _writer = self.writer.lock().unwrap();
        let stored = self.store.get_json(KEY)?.unwrap_or_default();
        let batch = self.pending.lock().unwrap();
        let mut paths = merge(batch.pending(), stored);
        if limit > 0 {
            paths.truncate(limit);
        }
        Ok(paths)
    }

    pub fn flush_due(&self, now: Instant) -> Result<bool> {
        self.flush_pending(Some(now))
    }

    pub fn flush(&self) -> Result<bool> {
        self.flush_pending(None)
    }

    fn flush_pending(&self, deadline: Option<Instant>) -> Result<bool> {
        let _writer = self.writer.lock().unwrap();
        let mut batch = {
            let mut pending = self.pending.lock().unwrap();
            if deadline.is_some_and(|now| !pending.is_due(now)) {
                return Ok(false);
            }
            pending.take_pending()
        };
        let result = batch.flush(|items| self.persist(items));
        if result.is_err() {
            self.pending.lock().unwrap().restore_pending(batch);
        }
        result
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/recent_files.rs"]
mod tests;
