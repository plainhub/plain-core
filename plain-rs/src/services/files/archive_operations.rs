use chrono::{Datelike, TimeZone, Timelike};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;

pub struct Cancel(pub Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
pub struct Output {
    path: PathBuf,
    directory: bool,
    committed: bool,
    files: Vec<String>,
}
impl Drop for Output {
    fn drop(&mut self) {
        if !self.committed {
            if self.directory {
                let _ = std::fs::remove_dir_all(&self.path);
            } else {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}
impl Output {
    pub fn keep(mut self) -> (PathBuf, Vec<String>) {
        self.committed = true;
        (self.path.clone(), std::mem::take(&mut self.files))
    }
}

pub async fn pack_named_files(
    files: Vec<(PathBuf, String)>,
    target: PathBuf,
    stop: watch::Receiver<bool>,
) -> Result<String, String> {
    if files.is_empty() {
        return Err("Archive sources required".into());
    }
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    let cancelled = cancel.0.clone();
    let output = tokio::task::spawn_blocking(move || {
        let mut names = HashSet::new();
        let mut entries = Vec::with_capacity(files.len());
        for (path, name) in files {
            if !path.is_absolute()
                || name.is_empty()
                || name.contains(['\\', '\0'])
                || Path::new(&name)
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || !names.insert(name.clone())
            {
                return Err("Invalid archive source or entry name".into());
            }
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if !metadata.is_file() {
                return Err("Regular archive source required".into());
            }
            entries.push(Entry {
                path,
                name,
                directory: false,
            });
        }
        pack_entries(entries, target, &stop, &cancelled)
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(output.keep().0.to_string_lossy().into_owned())
}

pub fn running(stop: &watch::Receiver<bool>, cancelled: &AtomicBool) -> Result<(), String> {
    if *stop.borrow() || stop.has_changed().is_err() || cancelled.load(Ordering::Acquire) {
        return Err("Archive operation cancelled".into());
    }
    Ok(())
}

pub fn copy(
    reader: &mut impl Read,
    writer: &mut impl Write,
    stop: &watch::Receiver<bool>,
    cancelled: &AtomicBool,
) -> Result<u64, String> {
    let mut buffer = [0; 64 * 1024];
    let mut total = 0;
    loop {
        running(stop, cancelled)?;
        let size = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if size == 0 {
            return Ok(total);
        }
        writer
            .write_all(&buffer[..size])
            .map_err(|error| error.to_string())?;
        total += size as u64;
    }
}

fn reserve(target: &Path, directory: bool) -> Result<(Output, Option<File>), String> {
    if !target.is_absolute() {
        return Err("Absolute archive destination required".into());
    }
    let parent = target
        .parent()
        .ok_or("Archive destination parent required")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    loop {
        let path = plain_rs::utils::unique_path::unique_sibling(target)
            .map_err(|error| error.to_string())?;
        let result = if directory {
            std::fs::create_dir(&path).map(|_| None)
        } else {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map(Some)
        };
        match result {
            Ok(file) => {
                return Ok((
                    Output {
                        path,
                        directory,
                        committed: false,
                        files: Vec::new(),
                    },
                    file,
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
}

pub struct Entry {
    pub path: PathBuf,
    name: String,
    directory: bool,
}

enum Visit {
    Enter(PathBuf, String),
    Leave(PathBuf),
}

pub fn collect_entries(
    paths: Vec<String>,
    stop: &watch::Receiver<bool>,
    cancelled: &AtomicBool,
) -> Result<Vec<Entry>, String> {
    running(stop, cancelled)?;
    let mut entries = Vec::new();
    let mut names = HashSet::new();
    for source in paths {
        let path = PathBuf::from(source);
        if !path.is_absolute() {
            return Err("Absolute archive source required".into());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Archive source name required")?
            .to_owned();
        let mut ancestors = HashSet::new();
        let mut pending = vec![Visit::Enter(path, name)];
        while let Some(visit) = pending.pop() {
            running(stop, cancelled)?;
            let (path, name) = match visit {
                Visit::Enter(path, name) => (path, name),
                Visit::Leave(path) => {
                    ancestors.remove(&path);
                    continue;
                }
            };
            let path = path.canonicalize().map_err(|error| error.to_string())?;
            let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
            if !names.insert(name.clone()) {
                return Err(format!("Duplicate archive entry: {name}"));
            }
            if metadata.is_dir() {
                if !ancestors.insert(path.clone()) {
                    return Err(format!(
                        "Archive source contains a directory cycle: {}",
                        path.display()
                    ));
                }
                pending.push(Visit::Leave(path.clone()));
                let mut children = std::fs::read_dir(&path)
                    .map_err(|error| error.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                children.sort_by_key(|entry| entry.file_name());
                for child in children.into_iter().rev() {
                    let file_name = child
                        .file_name()
                        .into_string()
                        .map_err(|_| "Archive source name is not UTF-8")?;
                    pending.push(Visit::Enter(child.path(), format!("{name}/{file_name}")));
                }
            } else if !metadata.is_file() {
                return Err("Regular archive source required".into());
            }
            entries.push(Entry {
                path,
                name,
                directory: metadata.is_dir(),
            });
        }
    }
    Ok(entries)
}

#[cfg(test)]
fn pack_files(
    paths: Vec<String>,
    target: PathBuf,
    stop: &watch::Receiver<bool>,
    cancelled: &AtomicBool,
) -> Result<Output, String> {
    pack_entries(
        collect_entries(paths, stop, cancelled)?,
        target,
        stop,
        cancelled,
    )
}

pub fn pack_entries(
    entries: Vec<Entry>,
    target: PathBuf,
    stop: &watch::Receiver<bool>,
    cancelled: &AtomicBool,
) -> Result<Output, String> {
    running(stop, cancelled)?;
    let (mut output, file) = reserve(&target, false)?;
    let mut zip = zip::ZipWriter::new(file.ok_or("Archive file missing")?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for Entry {
        path,
        name,
        directory,
    } in entries
    {
        running(stop, cancelled)?;
        if directory {
            zip.add_directory(format!("{name}/"), options)
                .map_err(|error| error.to_string())?;
        } else {
            let modified = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .map_err(|error| error.to_string())?;
            let date: chrono::DateTime<chrono::Local> = modified.into();
            let options = match zip::DateTime::from_date_and_time(
                date.year() as u16,
                date.month() as u8,
                date.day() as u8,
                date.hour() as u8,
                date.minute() as u8,
                date.second() as u8,
            ) {
                Ok(time) => options.last_modified_time(time),
                Err(_) => options,
            };
            zip.start_file(name, options)
                .map_err(|error| error.to_string())?;
            let mut source = File::open(path).map_err(|error| error.to_string())?;
            copy(&mut source, &mut zip, stop, cancelled)?;
        }
    }
    let file = zip.finish().map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    running(stop, cancelled)?;
    output
        .files
        .push(output.path.to_string_lossy().into_owned());
    Ok(output)
}

fn relative(name: &str) -> Result<PathBuf, String> {
    if name.contains('\0') || name.starts_with('/') {
        return Err("Archive entry escapes destination".into());
    }
    let mut result = PathBuf::new();
    for part in Path::new(name).components() {
        match part {
            Component::CurDir => {}
            Component::Normal(part) => result.push(part),
            Component::ParentDir if result.pop() => {}
            _ => return Err("Archive entry escapes destination".into()),
        }
    }
    if result.as_os_str().is_empty() {
        return Err("Archive entry name required".into());
    }
    Ok(result)
}

pub fn unpack_files(
    source: &Path,
    target: &Path,
    stop: &watch::Receiver<bool>,
    cancelled: &AtomicBool,
) -> Result<Output, String> {
    running(stop, cancelled)?;
    let file = File::open(source).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut entries = Vec::new();
    for index in 0..zip.len() {
        running(stop, cancelled)?;
        let entry = zip.by_index(index).map_err(|error| error.to_string())?;
        if entry
            .unix_mode()
            .is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o040000 | 0o100000))
        {
            return Err("Unsupported archive entry type".into());
        }
        entries.push(relative(&entry.name().map_err(|error| error.to_string())?)?);
    }
    let (mut output, _) = reserve(target, true)?;
    let root = output
        .path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut written = HashSet::new();
    for (index, name) in entries.into_iter().enumerate() {
        running(stop, cancelled)?;
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        let path = root.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            continue;
        }
        let parent = path.parent().ok_or("Archive entry parent required")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        if !parent
            .canonicalize()
            .map_err(|error| error.to_string())?
            .starts_with(&root)
        {
            return Err("Archive entry escapes destination".into());
        }
        let repeated = written.contains(&path);
        let mut options = OpenOptions::new();
        options.write(true);
        if repeated {
            options.truncate(true);
        } else {
            options.create_new(true);
        }
        let mut file = options.open(&path).map_err(|error| error.to_string())?;
        let size = copy(&mut entry, &mut file, stop, cancelled)?;
        if size != entry.size() {
            return Err("Archive entry size changed".into());
        }
        file.sync_all().map_err(|error| error.to_string())?;
        if let Some(date) = entry.last_modified() {
            if let Some(time) = chrono::NaiveDate::from_ymd_opt(
                i32::from(date.year()),
                u32::from(date.month()),
                u32::from(date.day()),
            )
            .and_then(|date_value| {
                date_value.and_hms_opt(
                    u32::from(date.hour()),
                    u32::from(date.minute()),
                    u32::from(date.second()),
                )
            })
            .and_then(|time| chrono::Local.from_local_datetime(&time).earliest())
            .filter(|time| time.timestamp() > 0)
            {
                file.set_modified(std::time::SystemTime::from(time))
                    .map_err(|error| error.to_string())?;
            }
        }
        if written.insert(path.clone()) {
            output.files.push(path.to_string_lossy().into_owned());
        }
    }
    running(stop, cancelled)?;
    Ok(output)
}

use plain_rs::services::files::platform::FilePlatform;
pub async fn pack(
    platform: &FilePlatform,
    paths: Vec<String>,
    target: String,
    stop: watch::Receiver<bool>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("Archive sources required".into());
    }
    platform
        .authorize_paths(&paths, false)
        .await
        .map_err(|error| error.to_string())?;
    platform
        .authorize(&target, true)
        .await
        .map_err(|error| error.to_string())?;
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    let cancelled = cancel.0.clone();
    let planning_stop = stop.clone();
    let entries =
        tokio::task::spawn_blocking(move || collect_entries(paths, &planning_stop, &cancelled))
            .await
            .map_err(|error| error.to_string())??;
    let mut authorized = HashSet::new();
    let paths = entries
        .iter()
        .filter(|entry| authorized.insert(entry.path.clone()))
        .map(|entry| entry.path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    platform
        .authorize_paths(&paths, false)
        .await
        .map_err(|error| error.to_string())?;
    let cancelled = cancel.0.clone();
    let output = tokio::task::spawn_blocking(move || {
        pack_entries(entries, PathBuf::from(target), &stop, &cancelled)
    })
    .await
    .map_err(|error| error.to_string())??;
    let (path, files) = output.keep();
    platform
        .scan(&files)
        .await
        .map_err(|error| format!("Archive saved at {}: {error}", path.display()))?;
    Ok(path.to_string_lossy().into_owned())
}

pub async fn unpack(
    platform: &FilePlatform,
    source: String,
    target: String,
    stop: watch::Receiver<bool>,
) -> Result<String, String> {
    platform
        .authorize(&source, false)
        .await
        .map_err(|error| error.to_string())?;
    platform
        .authorize(&target, true)
        .await
        .map_err(|error| error.to_string())?;
    let cancel = Cancel(Arc::new(AtomicBool::new(false)));
    let cancelled = cancel.0.clone();
    let output = tokio::task::spawn_blocking(move || {
        unpack_files(Path::new(&source), Path::new(&target), &stop, &cancelled)
    })
    .await
    .map_err(|error| error.to_string())??;
    let (path, files) = output.keep();
    platform
        .scan(&files)
        .await
        .map_err(|error| format!("Archive extracted at {}: {error}", path.display()))?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/archive_operations.rs"]
mod tests;
