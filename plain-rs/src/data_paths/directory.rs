use std::{
    collections::HashMap,
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

pub struct DataDirectory {
    _lock: File,
    cache: plain_rs::cache_paths::CachePaths,
}
impl DataDirectory {
    pub fn cache_paths(&self) -> &plain_rs::cache_paths::CachePaths {
        &self.cache
    }
    pub fn open(directory: &Path, cache_directory: &Path) -> Result<Arc<Self>, String> {
        if !directory.is_absolute() {
            return Err("Absolute data directory required".into());
        }
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let directory = directory
            .canonicalize()
            .map_err(|error| error.to_string())?;
        static OPEN: OnceLock<Mutex<HashMap<PathBuf, Arc<DataDirectory>>>> = OnceLock::new();
        let mut opened = OPEN
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|error| error.to_string())?;
        let paths = plain_rs::data_paths::DataPaths::new(&directory);
        if let Some(existing) = opened.get(&directory).cloned() {
            if existing.cache.root() != cache_directory {
                return Err("Data directory is already bound to another cache directory".into());
            }
            if paths
                .pending_restore()
                .try_exists()
                .map_err(|error| error.to_string())?
            {
                return Err("Restore requires a process restart".into());
            }
            return Ok(existing);
        }
        let lock_path = paths.lock();
        if std::fs::symlink_metadata(&lock_path).is_ok_and(|metadata| metadata.is_symlink()) {
            return Err("Invalid data directory lock".into());
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|error| error.to_string())?;
        lock.try_lock()
            .map_err(|error| format!("Data directory is already in use: {error}"))?;
        let cache = plain_rs::cache_paths::CachePaths::new(cache_directory);
        cache
            .initialize(&directory)
            .map_err(|error| error.to_string())?;
        if paths
            .pending_restore()
            .try_exists()
            .map_err(|error| error.to_string())?
        {
            std::fs::remove_dir_all(cache.root()).map_err(|error| error.to_string())?;
            cache
                .initialize(&directory)
                .map_err(|error| error.to_string())?;
        }
        plain_rs::services::backups::restore::apply(&directory)?;
        paths
            .initialize_directories()
            .map_err(|error| error.to_string())?;
        let directory_lock = Arc::new(Self { _lock: lock, cache });
        opened.insert(directory, directory_lock.clone());
        Ok(directory_lock)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/data_paths/directory.rs"]
mod tests;
