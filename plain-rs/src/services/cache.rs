use plain_rs::cache_paths::CachePaths;
use std::path::{Path, PathBuf};

fn cache(paths: &CachePaths) -> std::io::Result<Option<PathBuf>> {
    let root = paths.root().to_owned();
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => Ok(Some(root)),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid application cache directory",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn size(paths: &CachePaths) -> std::io::Result<u64> {
    let mut pending: Vec<PathBuf> = cache(paths)?.into_iter().collect();
    let mut size = 0u64;
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                size = size.checked_add(metadata.len()).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Cache size overflow")
                })?;
            }
        }
    }
    Ok(size)
}

pub fn clear_files(paths: &CachePaths, retained: Option<&Path>) -> std::io::Result<()> {
    let Some(root) = cache(paths)? else {
        return Ok(());
    };
    let media_caches = [paths.image_cache(), paths.video_cache()];
    clear_directory(&root, retained, &media_caches)?;
    Ok(())
}

fn clear_directory(
    directory: &Path,
    retained: Option<&Path>,
    media_caches: &[PathBuf],
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if retained.is_some_and(|retained| retained == directory || retained == path)
            || (media_caches.contains(&path) && metadata.is_dir())
        {
            continue;
        }
        let result =
            if metadata.is_dir() && retained.is_some_and(|retained| retained.starts_with(&path)) {
                clear_directory(&path, retained, media_caches)
            } else if metadata.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
        match result {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            result => result?,
        }
    }
    Ok(())
}

pub async fn clear(
    paths: &CachePaths,
    host: &plain_rs::host::bridge::Host,
    retained: Option<PathBuf>,
) -> Result<(), String> {
    let root = paths.clone();
    tokio::task::spawn_blocking(move || cache(&root))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    plain_rs::platform::media_cache::clear(paths, host).await?;
    let paths = paths.clone();
    tokio::task::spawn_blocking(move || clear_files(&paths, retained.as_deref()))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

pub async fn clear_application(state: &plain_rs::runtime::Runtime) -> Result<(), String> {
    let state = state.clone();
    tokio::spawn(async move {
        let _cache = state.cache_changes.write().await;
        let download = state.update_checks.download.clone();
        let _downloads = download.lock_for_cache_clear().await;
        download
            .reset_after_cache_clear(&state)
            .map_err(|error| error.to_string())?;
        let retained = state
            .web_root
            .read()
            .map_err(|error| error.to_string())?
            .clone();
        clear(&state.cache, &state.host, retained).await
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../tests/unit/services/cache.rs"]
mod tests;
