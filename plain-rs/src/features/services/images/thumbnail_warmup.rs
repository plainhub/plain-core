use super::thumbnails::Request;
use crate::features::enums::DataType;
use crate::infrastructure::db::{FileKind, MediaFileRow};
use crate::runtime::Runtime;
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::sync::{Mutex, atomic::Ordering};
use tokio::sync::mpsc;

const PENDING_LIMIT: usize = 320;
const TRIM_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) struct Schedule {
    sender: mpsc::Sender<Source>,
    receiver: Mutex<Option<mpsc::Receiver<Source>>>,
}

impl Default for Schedule {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel(PENDING_LIMIT);
        Self {
            sender,
            receiver: Mutex::new(Some(receiver)),
        }
    }
}

impl Schedule {
    pub(crate) async fn enqueue(
        &self,
        rows: &[MediaFileRow],
        scope: Option<&crate::features::prefs::file_index::Scope>,
    ) -> Result<(), String> {
        for row in rows {
            if scope.is_some_and(|scope| !scope.contains(std::path::Path::new(&row.path))) {
                continue;
            }
            if let Some(source) = Source::from_row(row) {
                self.sender
                    .send(source)
                    .await
                    .map_err(|_| "Thumbnail prewarming stopped")?;
            }
        }
        Ok(())
    }
}

struct Source {
    id: String,
    path: String,
    kind: FileKind,
}

impl Source {
    fn from_row(row: &MediaFileRow) -> Option<Self> {
        let kind = match row.data_type {
            value if value == DataType::Image.kind() => FileKind::Image,
            value if value == DataType::Audio.kind() => FileKind::Audio,
            value if value == DataType::Video.kind() => FileKind::Video,
            _ => return None,
        };
        (!row.trashed && std::path::Path::new(&row.path).is_absolute()).then(|| Self {
            id: row.id.clone(),
            path: row.path.clone(),
            kind,
        })
    }
}

pub(crate) fn start(state: Runtime) {
    let service = state.thumbnails.clone();
    let mut pending = state
        .file_platform
        .thumbnail_warmup
        .receiver
        .lock()
        .unwrap()
        .take()
        .unwrap();
    let task = tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let workers =
            plain_libs::media::thumbnail_limits::background_workers(state.thumbnails.workers());
        let mut running = FuturesUnordered::new();
        let mut batch = Vec::with_capacity(workers);
        if let Err(error) = trim(&state).await {
            log::warn!("Thumbnail cache trim: {error}");
        }
        while !*stop.borrow() {
            let slots = workers - running.len();
            tokio::select! {
                _ = stop.wait_for(|value| *value) => break,
                count = pending.recv_many(&mut batch, slots), if slots > 0 => {
                    if count == 0 { break; }
                    for source in batch.drain(..) {
                        if !state.host.uses_system_media_catalog() {
                            running.push(warm_source(&state, source));
                        }
                    }
                },
                _ = running.next(), if !running.is_empty() => {},
            }
            let written = state.thumbnails.cache_bytes_written.load(Ordering::Relaxed);
            if written >= TRIM_BYTES {
                match trim(&state).await {
                    Ok(()) => {
                        state
                            .thumbnails
                            .cache_bytes_written
                            .fetch_sub(written, Ordering::Relaxed);
                    }
                    Err(error) => log::warn!("Thumbnail cache trim: {error}"),
                }
            }
        }
    });
    *service.warmer.lock().unwrap() = Some(task);
}

async fn warm_source(state: &Runtime, row: Source) {
    let grid = if row.kind == FileKind::Image {
        256
    } else {
        512
    };
    for size in [96, grid] {
        let request = Request {
            file_name: plain_libs::utils::query::url_encode(
                row.path.rsplit('/').next().unwrap_or_default(),
            ),
            path: row.path.clone(),
            file_id: row.id.clone(),
            width: size,
            height: size,
            center_crop: true,
        };
        match state.thumbnails.get_inner(state, request, true).await {
            Ok(_) => {}
            Err(error) => log::warn!("Thumbnail prewarm generation: {error}"),
        }
    }
}

async fn trim(state: &Runtime) -> Result<(), String> {
    let cache = state.cache.root().join("thumbs");
    tokio::task::spawn_blocking(move || trim_cache(&cache))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

fn trim_cache(directory: &std::path::Path) -> std::io::Result<()> {
    let mut directories = vec![directory.to_owned()];
    let mut files = Vec::new();
    let mut total = 0;
    while let Some(directory) = directories.pop() {
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Thumbnail cache directory is a symlink",
            ));
        }
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.push(entry.path());
            } else if kind.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "thumb" || extension == "jpg")
            {
                let metadata = entry.metadata()?;
                total += metadata.len();
                files.push((metadata.modified()?, metadata.len(), entry.path()));
            }
        }
    }
    files.sort_unstable_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in files {
        if total <= 256 * 1024 * 1024 {
            break;
        }
        std::fs::remove_file(path)?;
        total -= size;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/images/thumbnail_warmup.rs"]
mod tests;
