use super::Cancel;
use crate::{
    host::resources::NativeResources,
    services::files::archive_operations::{copy, running},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{Read, Seek, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::AtomicBool,
    },
};
use tokio::sync::watch;

const STAGING_PREFIX: &str = ".restore-stage-";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    staging: String,
    entries: Vec<String>,
}
struct Publication {
    path: PathBuf,
    committed: bool,
}
impl Drop for Publication {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn reserved(name: &str) -> bool {
    name == ".core.lock" || name.starts_with(".restore-")
}
fn component(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.contains(['\\', '\0'])
        || Path::new(value).components().count() != 1
        || !matches!(
            Path::new(value).components().next(),
            Some(Component::Normal(_))
        )
    {
        return Err("Invalid restore path".into());
    }
    Ok(())
}
fn relative(name: &str) -> Result<PathBuf, String> {
    let name = name
        .strip_prefix("files/")
        .ok_or("Backup does not match the current data layout")?;
    let name = name.strip_suffix('/').unwrap_or(name);
    if name.is_empty() || name.contains(['\\', '\0']) {
        return Err("Invalid restore entry".into());
    }
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("Invalid restore entry".into());
    }
    let path = PathBuf::from(name);
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("Invalid restore entry".into());
    }
    let top = path
        .components()
        .next()
        .unwrap()
        .as_os_str()
        .to_str()
        .ok_or("Invalid restore entry")?;
    if reserved(top) || ["plain.db-wal", "plain.db-shm", "plain.db-journal"].contains(&top) {
        return Err("Backup contains runtime files".into());
    }
    Ok(path)
}
fn exists(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}
fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = path;
        Ok(())
    }
}
fn sync_tree(path: &Path) -> Result<(), String> {
    let mut pending = vec![path.to_path_buf()];
    let mut directories = Vec::new();
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.is_symlink() {
            return Err("Restore payload contains a symbolic link".into());
        }
        if metadata.is_dir() {
            directories.push(path.clone());
            for child in std::fs::read_dir(&path).map_err(|error| error.to_string())? {
                pending.push(child.map_err(|error| error.to_string())?.path());
            }
        } else if metadata.is_file() {
            File::open(path)
                .and_then(|file| file.sync_all())
                .map_err(|error| error.to_string())?;
        } else {
            return Err("Restore payload contains a special file".into());
        }
    }
    for directory in directories.into_iter().rev() {
        sync_directory(&directory)?;
    }
    Ok(())
}
fn validate(payload: &Path) -> Result<(), String> {
    let paths = plain_rs::data_paths::DataPaths::new(payload);
    for path in [
        paths.database(),
        paths.prefs(),
        paths.kv().join("version"),
    ] {
        if !std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file()) {
            return Err("Backup is missing a required data file".into());
        }
    }
    let database = rusqlite::Connection::open_with_flags(
        paths.database(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|error| error.to_string())?;
    let check: String = database
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if check != "ok" {
        return Err(format!("Invalid backup database: {check}"));
    }
    let prefs = plain_rs::prefs::Prefs::load(&paths.prefs())
        .map_err(|error| error.to_string())?;
    plain_rs::prefs::settings::snapshot(&prefs).map_err(|error| error.to_string())?;
    plain_rs::prefs::security::snapshot(&prefs).map_err(|error| error.to_string())?;
    plain_rs::kv::Db::open(&paths.kv())
        .and_then(|database| database.flush())
        .map_err(|error| error.to_string())?;
    Ok(())
}
fn stage_reader(
    source: impl Read + Seek,
    directory: &Path,
    stop: &watch::Receiver<bool>,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    running(stop, cancelled)?;
    let paths = plain_rs::data_paths::DataPaths::new(directory);
    if exists(&paths.pending_restore())? {
        return Err("A restore is already pending".into());
    }
    let staging = tempfile::Builder::new()
        .prefix(STAGING_PREFIX)
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    let payload = staging.path().join("payload");
    std::fs::create_dir(&payload).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(source).map_err(|error| error.to_string())?;
    let mut names = std::collections::HashSet::new();
    let mut entries = BTreeSet::new();
    for index in 0..zip.len() {
        running(stop, cancelled)?;
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        let name = relative(&entry.name().map_err(|error| error.to_string())?)?;
        if !names.insert(name.clone()) {
            return Err("Backup contains duplicate entries".into());
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err("Backup contains a special file".into());
            }
        }
        entries.insert(
            name.components()
                .next()
                .unwrap()
                .as_os_str()
                .to_str()
                .ok_or("Invalid restore entry")?
                .to_owned(),
        );
        let target = payload.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        } else {
            std::fs::create_dir_all(target.parent().unwrap()).map_err(|error| error.to_string())?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|error| error.to_string())?;
            copy(&mut entry, &mut output, stop, cancelled)?;
            output.sync_all().map_err(|error| error.to_string())?;
        }
    }
    running(stop, cancelled)?;
    validate(&payload)?;
    sync_tree(&payload)?;
    std::fs::create_dir(staging.path().join("old")).map_err(|error| error.to_string())?;
    sync_directory(staging.path())?;
    let pending = Pending {
        staging: staging
            .path()
            .file_name()
            .unwrap()
            .to_str()
            .ok_or("Invalid staging directory")?
            .to_owned(),
        entries: entries.into_iter().collect(),
    };
    let mut marker = tempfile::Builder::new()
        .prefix(".restore-marker-")
        .tempfile_in(directory)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(marker.as_file_mut(), &pending).map_err(|error| error.to_string())?;
    marker
        .as_file_mut()
        .flush()
        .and_then(|_| marker.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    running(stop, cancelled)?;
    let mut publication = Publication {
        path: staging.keep(),
        committed: false,
    };
    marker
        .persist_noclobber(paths.pending_restore())
        .map_err(|error| error.to_string())?;
    publication.committed = true;
    sync_directory(directory)
}

pub(crate) fn apply(directory: &Path) -> Result<(), String> {
    let paths = plain_rs::data_paths::DataPaths::new(directory);
    if !exists(&paths.pending_restore())? {
        return cleanup(directory);
    }
    if std::fs::symlink_metadata(paths.pending_restore())
        .map_err(|error| error.to_string())?
        .is_symlink()
    {
        return Err("Invalid restore marker".into());
    }
    let pending: Pending = serde_json::from_reader(
        File::open(paths.pending_restore()).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    component(&pending.staging)?;
    if !pending.staging.starts_with(STAGING_PREFIX) {
        return Err("Invalid restore staging directory".into());
    }
    let staging = directory.join(&pending.staging);
    for path in [&staging, &staging.join("payload"), &staging.join("old")] {
        if !std::fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.is_symlink())
        {
            return Err("Invalid restore staging directory".into());
        }
    }
    let mut names = BTreeSet::<String>::new();
    for name in &pending.entries {
        component(name)?;
        if reserved(name)
            || ["plain.db-wal", "plain.db-shm", "plain.db-journal"].contains(&name.as_str())
            || !names.insert(name.clone())
        {
            return Err("Invalid restore plan".into());
        }
    }
    if !["plain.db", "prefs.json", "fjall"]
        .iter()
        .all(|name| names.contains(*name))
    {
        return Err("Incomplete restore plan".into());
    }
    sync_tree(&staging.join("payload"))?;
    for runtime in ["plain.db-wal", "plain.db-shm", "plain.db-journal"] {
        let source = directory.join(runtime);
        let target = staging.join("old").join(runtime);
        if exists(&source)? {
            if exists(&target)? {
                return Err("Restore runtime paths changed during startup".into());
            }
            std::fs::rename(source, target).map_err(|error| error.to_string())?;
        }
    }
    sync_directory(directory)?;
    sync_directory(&staging.join("old"))?;
    for name in pending.entries {
        let source = staging.join("payload").join(&name);
        let target = directory.join(&name);
        let old = staging.join("old").join(&name);
        if exists(&source)? {
            if exists(&target)? {
                if exists(&old)? {
                    return Err("Restore target changed during startup".into());
                }
                std::fs::rename(&target, &old).map_err(|error| error.to_string())?;
                sync_directory(directory)?;
                sync_directory(&staging.join("old"))?;
            }
            std::fs::rename(&source, &target).map_err(|error| error.to_string())?;
            sync_directory(directory)?;
            sync_directory(&staging.join("payload"))?;
        } else if !exists(&target)? {
            return Err("Restore payload is incomplete".into());
        }
    }
    std::fs::remove_file(paths.pending_restore()).map_err(|error| error.to_string())?;
    sync_directory(directory)?;
    std::fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    cleanup(directory)
}
fn cleanup(directory: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if name.to_str().is_some_and(|name| {
            name.starts_with(STAGING_PREFIX) || name.starts_with(".restore-marker-")
        }) {
            let metadata =
                std::fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            if metadata.is_dir() && !metadata.is_symlink() {
                std::fs::remove_dir_all(entry.path()).map_err(|error| error.to_string())?;
            } else {
                std::fs::remove_file(entry.path()).map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

pub(crate) async fn stage(
    path: String,
    directory: PathBuf,
    cache: PathBuf,
    resources: Arc<NativeResources>,
    mut stop: watch::Receiver<bool>,
) -> Result<(), String> {
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    running(&stop, &cancel.0)?;
    if exists(&plain_rs::data_paths::DataPaths::new(&directory).pending_restore())? {
        return Err("A restore is already pending".into());
    }
    let cancelled = cancel.0.clone();
    let stopped = stop.clone();
    let preparation =
        crate::services::files::input::read_source(&resources, &cache, &path, move |source| {
            stage_reader(source, &directory, &stopped, &cancelled)
        });
    tokio::select! { _=stop.changed()=>Err("Restore cancelled".into()), result=preparation=>result }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/backups/restore.rs"]
mod tests;
