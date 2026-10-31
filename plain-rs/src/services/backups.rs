pub mod restore;
use plain_rs::{db::Db, prefs::Prefs};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;

struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
struct Roots {
    folders: Vec<(PathBuf, String)>,
    cache: PathBuf,
}
fn roots(directory: &Path, cache: &Path) -> Roots {
    Roots {
        folders: vec![(directory.to_path_buf(), "files".into())],
        cache: cache.to_owned(),
    }
}
fn name(path: &Path, roots: &[(PathBuf, String)]) -> Result<String, String> {
    let (root, prefix) = roots
        .iter()
        .filter(|(root, _)| path.starts_with(root))
        .max_by_key(|(root, _)| root.components().count())
        .ok_or("Backup source is outside storage roots")?;
    let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
    let mut result = prefix.clone();
    for part in relative.components() {
        let std::path::Component::Normal(part) = part else {
            return Err("Invalid backup path".into());
        };
        let part = part.to_str().ok_or("Backup path must be UTF-8")?;
        if part.contains(['\\', '\0']) {
            return Err("Invalid backup path".into());
        }
        result.push('/');
        result.push_str(part);
    }
    Ok(result)
}
fn collect(
    folder: &Path,
    roots: &[(PathBuf, String)],
    excluded: &HashSet<PathBuf>,
    files: &mut Vec<(PathBuf, String)>,
    cancelled: &impl Fn() -> bool,
) -> Result<(), String> {
    if cancelled() {
        return Err("Backup cancelled".into());
    }
    if roots.iter().any(|(root, _)| {
        folder.parent() == Some(root.as_path())
            && folder
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(restore::reserved)
    }) {
        return Ok(());
    }
    if excluded.contains(folder) {
        return Ok(());
    }
    let metadata = match std::fs::symlink_metadata(folder) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if metadata.is_symlink() {
        return Ok(());
    }
    if metadata.is_file() {
        files.push((folder.to_path_buf(), name(folder, roots)?));
    } else if metadata.is_dir() {
        let mut children = std::fs::read_dir(folder)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            collect(&child.path(), roots, excluded, files, cancelled)?;
        }
    }
    Ok(())
}
fn snapshot(
    db: &Db,
    prefs: &Prefs,
    directory: &Path,
    roots: &Roots,
    cancelled: impl Fn() -> bool,
) -> Result<(tempfile::TempDir, Vec<(PathBuf, String)>), String> {
    if cancelled() {
        return Err("Backup cancelled".into());
    }
    std::fs::create_dir_all(&roots.cache).map_err(|error| error.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix("backup-")
        .tempdir_in(&roots.cache)
        .map_err(|error| error.to_string())?;
    let database_path = db
        .with_conn(|conn| conn.path().map(PathBuf::from))
        .ok_or("Persistent database required")?;
    let source_db = plain_rs::data_paths::DataPaths::new(directory).database();
    if database_path
        .canonicalize()
        .map_err(|error| error.to_string())?
        != source_db
            .canonicalize()
            .map_err(|error| error.to_string())?
    {
        return Err("Backup database is outside the data directory".into());
    }
    let mut excluded = HashSet::from([
        source_db.clone(),
        prefs.path().to_path_buf(),
        roots.cache.clone(),
    ]);
    for suffix in ["-wal", "-shm", "-journal"] {
        excluded.insert(PathBuf::from(format!(
            "{}{suffix}",
            source_db.to_string_lossy()
        )));
    }
    let mut files = Vec::new();
    let database = staging.path().join("database.db");
    db.backup_to(&database, &cancelled)?;
    files.push((database, name(&source_db, &roots.folders)?));
    for (index, path, entries) in [(0, prefs.path(), prefs.entries())] {
        if cancelled() {
            return Err("Backup cancelled".into());
        }
        let target = staging.path().join(format!("prefs-{index}.json"));
        let values: serde_json::Map<String, serde_json::Value> = entries.into_iter().collect();
        std::fs::write(
            &target,
            serde_json::to_vec_pretty(&values).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        files.push((target, name(path, &roots.folders)?));
    }
    for (folder, _) in &roots.folders {
        collect(folder, &roots.folders, &excluded, &mut files, &cancelled)?;
    }
    let mut names = HashSet::new();
    if !files.iter().all(|(_, name)| names.insert(name.clone())) {
        return Err("Overlapping backup roots".into());
    }
    Ok((staging, files))
}
pub async fn export(
    db: Arc<Db>,
    prefs: Arc<Prefs>,
    directory: PathBuf,
    cache: PathBuf,
    mut stop: watch::Receiver<bool>,
) -> Result<String, String> {
    let roots = roots(&directory, &cache);
    let output = roots.cache.join("backup.zip");
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    let cancelled = cancel.0.clone();
    let stopped = stop.clone();
    let preparation = tokio::task::spawn_blocking(move || {
        snapshot(&db, &prefs, &directory, &roots, || {
            cancelled.load(Ordering::Acquire) || *stopped.borrow()
        })
    });
    let (staging, files) = tokio::select! {
        _ = stop.changed() => return Err("Backup cancelled".into()),
        result = preparation => result.map_err(|error| error.to_string())??,
    };
    let archive =
        plain_rs::services::files::archive_operations::pack_named_files(files, output, stop).await;
    drop(staging);
    archive
}

#[cfg(test)]
#[path = "../../tests/unit/services/backups.rs"]
mod tests;

#[cfg(any(test, feature = "test-seams"))]
pub mod test_fixture {
    use super::*;
    use std::{
        io::{Cursor, Write},
        sync::Arc,
    };

    pub fn archive() -> Vec<u8> {
        let source = tempfile::tempdir().unwrap();
        let paths = plain_rs::data_paths::DataPaths::new(source.path());
        let prefs = Arc::new(plain_rs::prefs::Prefs::load(&paths.prefs()).unwrap());
        plain_rs::prefs::identity::ensure_identity(&prefs);
        prefs.set("device_name", "restored device").unwrap();
        let db = plain_rs::db::Db::open(&paths.database()).unwrap();
        db.with_conn(|conn| conn.execute_batch("CREATE TABLE restore_fixture (value TEXT); INSERT INTO restore_fixture VALUES ('snapshot');").unwrap());
        let store = plain_rs::kv::LazyDb::new(paths.database());
        store.get().unwrap().insert("restored", "snapshot").unwrap();
        std::fs::create_dir(source.path().join("files")).unwrap();
        std::fs::write(source.path().join("files/blob"), b"attachment").unwrap();
        let (_staging, files) = snapshot(
            &db,
            &prefs,
            source.path(),
            &roots(source.path(), &source.path().join("cache")),
            || false,
        )
        .unwrap();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (path, name) in files {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&std::fs::read(path).unwrap()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
}
