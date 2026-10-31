use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};

pub struct Archives {
    previews: Arc<plain_rs::utils::keyed_locks::KeyedLocks>,
    directory: PathBuf,
    gate: Arc<Semaphore>,
}

impl Archives {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            previews: Arc::new(plain_rs::utils::keyed_locks::KeyedLocks::new()),
            directory,
            gate: Arc::new(Semaphore::new(1)),
        }
    }

    pub async fn preview(
        &self,
        platform: &plain_rs::services::files::platform::FilePlatform,
        reference: String,
    ) -> Result<PathBuf, String> {
        let (archive, _) = reference
            .split_once("!zip!/")
            .ok_or("Archive entry reference required")?;
        platform
            .authorize(archive, false)
            .await
            .map_err(|error| error.to_string())?;
        let (_, name) = reference
            .split_once("!zip!/")
            .ok_or("Archive entry reference required")?;
        let reference = format!("{archive}!zip!/{}", name.trim_matches('/'));
        let cache = self.directory.join("entries");
        let previews = self.previews.clone();
        tokio::spawn(async move {
            previews
                .with_lock(reference.clone(), async move {
                    tokio::task::spawn_blocking(move || {
                        plain_rs::services::files::zip_entries::extract(&cache, &reference)
                    })
                    .await
                    .map_err(|error| error.to_string())?
                })
                .await
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.gate.clone().try_acquire_owned().ok()
    }

    pub async fn build(
        &self,
        items: Vec<(PathBuf, String)>,
        permit: OwnedSemaphorePermit,
        stop: watch::Receiver<bool>,
    ) -> Result<ArchiveReader, String> {
        self.build_with_inputs(items, permit, stop, ()).await
    }

    pub async fn build_with_inputs<Inputs: Send + 'static>(
        &self,
        items: Vec<(PathBuf, String)>,
        permit: OwnedSemaphorePermit,
        stop: watch::Receiver<bool>,
        input_guard: Inputs,
    ) -> Result<ArchiveReader, String> {
        ensure_running(&stop)?;
        let directory = self.directory.clone();
        let (file, staging, size, permit) = tokio::task::spawn_blocking(move || {
            let _input_guard = input_guard;
            let _permit = permit;
            let (file, staging) = build_archive(&directory, items)?;
            let size = file.metadata()?.len();
            Ok::<_, anyhow::Error>((file, staging, size, _permit))
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        let reader = ArchiveReader {
            file: Some(tokio::fs::File::from_std(file)),
            _staging: staging,
            size,
            _permit: permit,
            stop,
        };
        ensure_running(&reader.stop)?;
        Ok(reader)
    }
}

struct StagedFile(PathBuf);
impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub struct ArchiveReader {
    file: Option<tokio::fs::File>,
    _staging: StagedFile,
    pub size: u64,
    _permit: OwnedSemaphorePermit,
    stop: watch::Receiver<bool>,
}

impl ArchiveReader {
    pub async fn read(&mut self) -> Result<Option<Vec<u8>>, String> {
        use tokio::io::AsyncReadExt;
        ensure_running(&self.stop)?;
        let Some(file) = self.file.as_mut() else {
            return Ok(None);
        };
        let mut buffer = vec![0; 64 * 1024];
        let read = match file.read(&mut buffer).await {
            Ok(read) => read,
            Err(error) => {
                self.file.take();
                return Err(error.to_string());
            }
        };
        if read == 0 {
            self.file.take();
            return Ok(None);
        }
        buffer.truncate(read);
        Ok(Some(buffer))
    }
}

impl Drop for ArchiveReader {
    fn drop(&mut self) {
        self.file.take();
    }
}

fn ensure_running(stop: &watch::Receiver<bool>) -> Result<(), String> {
    if *stop.borrow() || stop.has_changed().is_err() {
        return Err("Rust core stopped".into());
    }
    Ok(())
}

fn build_archive(
    directory: &Path,
    items: Vec<(PathBuf, String)>,
) -> anyhow::Result<(std::fs::File, StagedFile)> {
    anyhow::ensure!(!items.is_empty(), "nothing to zip");
    let items = items
        .into_iter()
        .map(|(source, name)| {
            let name = if name.is_empty() {
                source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid archive entry name"))?
                    .to_owned()
            } else {
                name
            };
            anyhow::ensure!(
                !name.is_empty()
                    && !name.contains(['\\', '\0', ':'])
                    && name.split('/').all(|part| !matches!(part, "." | ".."))
                    && Path::new(&name)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_))),
                "Invalid archive entry name"
            );
            Ok((source, name))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    std::fs::create_dir_all(directory)?;
    let path = directory.join(format!("archive-{}.tmp", uuid::Uuid::new_v4()));
    let staging = StagedFile(path.clone());
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = vec![0; 64 * 1024];
    for (source, entry_name) in items {
        if source.is_dir() {
            writer.add_directory(format!("{}/", entry_name.trim_end_matches('/')), options)?;
            continue;
        }
        let mut reader = std::fs::File::open(&source)?;
        writer.start_file(entry_name, options)?;
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read])?;
        }
    }
    let mut file = writer.finish()?;
    file.seek(SeekFrom::Start(0))?;
    Ok((file, staging))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/archives.rs"]
mod tests;
