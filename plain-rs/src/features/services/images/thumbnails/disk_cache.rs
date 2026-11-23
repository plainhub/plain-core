use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};
use tokio::sync::Notify;

use super::CoreRuntime;

pub(super) const LIMIT: u64 = 1_000_000_000;

#[derive(Default)]
struct Usage {
    stored: Option<u64>,
    reserved: u64,
    writers: HashSet<PathBuf>,
}

pub(super) struct DiskCache {
    limit: u64,
    usage: Mutex<Usage>,
    wake: Notify,
}

pub(super) struct Reservation {
    cache: Arc<DiskCache>,
    path: PathBuf,
    size: u64,
}

impl DiskCache {
    pub(super) fn new(limit: u64) -> Self {
        Self {
            limit,
            usage: Mutex::new(Usage::default()),
            wake: Notify::new(),
        }
    }

    pub(super) fn reserve(self: &Arc<Self>, path: PathBuf, size: u64) -> Option<Reservation> {
        let mut usage = self.usage.lock().unwrap();
        let stored = usage.stored?;
        if size == 0 || size > self.limit {
            return None;
        }
        if stored.saturating_add(usage.reserved).saturating_add(size) > self.limit {
            self.wake.notify_one();
            return None;
        }
        if !usage.writers.insert(path.clone()) {
            return None;
        }
        usage.reserved += size;
        Some(Reservation {
            cache: self.clone(),
            path,
            size,
        })
    }

    pub(super) fn reset(&self) {
        self.usage.lock().unwrap().stored = None;
        self.wake.notify_one();
    }

    pub(super) async fn run(state: CoreRuntime) {
        let mut stop = state.stop.clone();
        loop {
            let guard = tokio::select! {
                biased;
                _ = stop.wait_for(|value| *value) => return,
                guard = state.cache_changes.clone().read_owned() => guard,
            };
            let directory = state.cache.thumbnails();
            let cache = state.thumbnails.disk_cache.clone();
            match tokio::task::spawn_blocking(move || {
                let _guard = guard;
                cache.maintain(&directory)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => log::warn!("Unable to maintain thumbnail cache: {error}"),
                Err(error) => log::warn!("Thumbnail cache maintenance did not finish: {error}"),
            }
            tokio::select! {
                biased;
                _ = stop.wait_for(|value| *value) => return,
                _ = state.thumbnails.disk_cache.wake.notified() => {}
            }
        }
    }

    fn maintain(&self, directory: &Path) -> io::Result<()> {
        let needs_scan = self.usage.lock().unwrap().stored.is_none();
        let target = self.limit * 4 / 5;
        if !needs_scan && self.total() <= target {
            return Ok(());
        }
        let mut entries = Vec::new();
        let mut stored = 0u64;
        visit_files(directory, |path, metadata| {
            let size = metadata.len();
            if needs_scan {
                stored = stored
                    .checked_add(size)
                    .ok_or_else(|| io::Error::other("Thumbnail cache size overflow"))?;
            }
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((modified, path, size));
            Ok(())
        })?;
        if needs_scan {
            self.usage.lock().unwrap().stored = Some(stored);
            if stored <= self.limit {
                return Ok(());
            }
        }
        entries.sort_unstable();
        for (_, path, size) in entries {
            if self.total() <= target {
                break;
            }
            if self.usage.lock().unwrap().writers.contains(&path) {
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    log::warn!("Unable to remove a cached thumbnail: {error}");
                    continue;
                }
            }
            let mut usage = self.usage.lock().unwrap();
            if let Some(stored) = &mut usage.stored {
                *stored = stored.saturating_sub(size);
            }
        }
        Ok(())
    }

    pub(super) fn total(&self) -> u64 {
        let usage = self.usage.lock().unwrap();
        usage.stored.unwrap_or(0).saturating_add(usage.reserved)
    }
}

impl Reservation {
    pub(super) fn persist(mut self, bytes: &[u8]) -> io::Result<()> {
        fs::create_dir_all(self.path.parent().unwrap())?;
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", plain_libs::utils::uuid::Uuid::new_v4()));
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        let temporary = Temporary(temporary);
        file.write_all(bytes)?;
        file.flush()?;
        drop(file);
        fs::rename(&temporary.0, &self.path)?;
        let mut usage = self.cache.usage.lock().unwrap();
        usage.reserved -= self.size;
        usage.writers.remove(&self.path);
        *usage
            .stored
            .as_mut()
            .expect("reservation prevents a cache reset") += self.size;
        self.size = 0;
        Ok(())
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.size != 0 {
            let mut usage = self.cache.usage.lock().unwrap();
            usage.reserved -= self.size;
            usage.writers.remove(&self.path);
        }
    }
}

struct Temporary(PathBuf);

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn visit_files(
    root: &Path,
    mut visit: impl FnMut(PathBuf, fs::Metadata) -> io::Result<()>,
) -> io::Result<()> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err(io::Error::other("Invalid thumbnail cache directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file()
                && matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("jpg" | "png")
                )
            {
                visit(path, metadata)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/images/thumbnails/disk_cache.rs"]
mod tests;
