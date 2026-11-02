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
        }
    }

    pub fn add(&self, path: &str) -> Result<()> {
        let now = Instant::now();
        let mut batch = self.pending.lock().unwrap();
        batch.push(path.to_owned(), now);
        batch.flush_due(now, |pending| self.persist(pending))?;
        Ok(())
    }

    fn persist(&self, pending: &[String]) -> Result<()> {
        self.store.update_json(KEY, |stored: &mut Vec<String>| {
            *stored = merge(pending, std::mem::take(stored));
            Ok(())
        })
    }

    pub fn list(&self, limit: usize) -> Result<Vec<String>> {
        let batch = self.pending.lock().unwrap();
        let stored = self.store.get_json(KEY)?.unwrap_or_default();
        let mut paths = merge(batch.pending(), stored);
        if limit > 0 {
            paths.truncate(limit);
        }
        Ok(paths)
    }

    pub fn flush_due(&self, now: Instant) -> Result<bool> {
        self.pending
            .lock()
            .unwrap()
            .flush_due(now, |pending| self.persist(pending))
    }

    pub fn flush(&self) -> Result<bool> {
        self.pending
            .lock()
            .unwrap()
            .flush(|pending| self.persist(pending))
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/recent_files.rs"]
mod tests;
