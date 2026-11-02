use super::thumbnails::Request;
use crate::infrastructure::db::{Db, FileKind};
use crate::runtime::Runtime;
use futures_util::{StreamExt, stream};

pub(crate) fn start(state: Runtime) {
    let mut events = state.events.subscribe();
    let mut stop = state.stop.clone();
    let service = state.thumbnails.clone();
    let task = tokio::spawn(async move {
        while !*stop.borrow() {
            let changed = tokio::select! {
                _ = stop.wait_for(|value| *value) => break,
                event = events.recv() => match event {
                    Ok(event) => event.event_type == crate::infrastructure::events::MEDIA_LIBRARY_UPDATED,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => true,
                    Err(_) => break,
                },
            };
            if !changed || state.host.uses_system_media_catalog() {
                continue;
            }
            while matches!(
                events.try_recv(),
                Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
            ) {}
            if let Err(error) = warm_catalog(&state).await {
                log::warn!("Thumbnail prewarm: {error}");
            }
        }
    });
    *service.warmer.lock().unwrap() = Some(task);
}

async fn warm_catalog(state: &Runtime) -> Result<(), String> {
    trim(state).await?;
    let workers = background_workers(std::thread::available_parallelism().map_or(1, |n| n.get()));
    let mut warmed_bytes = 0;
    let mut budget_available = true;
    for kind in [FileKind::Image, FileKind::Audio, FileKind::Video] {
        let mut before = None;
        let mut stop = state.stop.clone();
        while !*stop.borrow() && budget_available {
            state.thumbnails.wait_for_foreground(state).await?;
            let db = state.db.clone();
            let cursor = before.take();
            let rows = tokio::task::spawn_blocking(move || page(&db, kind, cursor.as_ref()))
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            let count = rows.len();
            before = rows.last().map(|row| Cursor {
                updated_at: row.updated_at.clone(),
                id: row.id.clone(),
            });
            let requests = stream::iter(rows)
                .map(|row| warm_source(state, row, kind))
                .buffer_unordered(workers);
            tokio::pin!(requests);
            loop {
                let bytes = tokio::select! {
                    _ = stop.wait_for(|value| *value) => return Ok(()),
                    result = requests.next() => result,
                };
                let Some(bytes) = bytes else {
                    break;
                };
                warmed_bytes += bytes;
                if warmed_bytes >= 128 * 1024 * 1024 {
                    budget_available = false;
                    break;
                }
            }
            if count < 32 {
                break;
            }
        }
    }
    trim(state).await
}

fn background_workers(cores: usize) -> usize {
    cores.saturating_sub(1).clamp(1, 3)
}

async fn warm_source(state: &Runtime, row: Source, kind: FileKind) -> usize {
    if !std::path::Path::new(&row.path).is_absolute() {
        return 0;
    }
    let mut total = 0;
    let grid = if kind == FileKind::Image { 256 } else { 512 };
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
            Ok(Some(bytes)) => total += bytes.len(),
            Ok(None) => {}
            Err(error) => log::warn!("Thumbnail prewarm generation: {error}"),
        }
    }
    total
}

struct Cursor {
    updated_at: String,
    id: String,
}
struct Source {
    id: String,
    path: String,
    updated_at: String,
}
fn page(db: &Db, kind: FileKind, before: Option<&Cursor>) -> rusqlite::Result<Vec<Source>> {
    db.with_conn(|connection| {
        let select = |condition: &str, arguments: &[&dyn rusqlite::ToSql], limit: usize| -> rusqlite::Result<Vec<Source>> {
            let sql = format!("SELECT f.id,rtrim(v.path,'/')||'/'||f.path,f.updated_at FROM files f JOIN volumes v ON v.id=f.volume_id WHERE f.kind=?1 AND f.trashed_at IS NULL AND f.missing=0 AND f.indexed=1 AND v.is_available=1 {condition} ORDER BY f.updated_at DESC,f.id DESC LIMIT {limit}");
            let mut statement = connection.prepare(&sql)?;
            statement.query_map(arguments, |row| Ok(Source { id:row.get(0)?,path:row.get(1)?,updated_at:row.get(2)? }))?.collect()
        };
        let kind = kind.as_str();
        let Some(before) = before else {
            return select("", &[&kind], 32);
        };
        let mut rows = select("AND f.updated_at=?2 AND f.id<?3", &[&kind,&before.updated_at,&before.id], 32)?;
        if rows.len()<32 {
            rows.extend(select("AND f.updated_at<?2", &[&kind,&before.updated_at], 32-rows.len())?);
        }
        Ok(rows)
    })
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
