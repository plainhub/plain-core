use super::io::Temporary;
use plain_libs::anyhow::{Result, ensure};
use std::{
    collections::HashSet,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct Item {
    pub source_path: PathBuf,
    pub entry_name: String,
}
pub(super) struct Cancel(pub Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
pub(super) fn pack(
    directory: PathBuf,
    items: Vec<Item>,
    canceled: Arc<AtomicBool>,
) -> Result<(std::fs::File, Temporary)> {
    ensure!(
        !items.is_empty() && items.len() <= 100_000,
        "Invalid download ZIP selection"
    );
    let mut names = HashSet::new();
    for item in &items {
        ensure!(
            item.source_path.is_absolute(),
            "Invalid download ZIP source"
        );
        ensure!(
            !item.entry_name.is_empty()
                && item.entry_name.split('/').all(|part| !part.is_empty()
                    && part != "."
                    && part != ".."
                    && !part.contains(['\\', '\0'])),
            "Invalid download ZIP entry"
        );
        ensure!(
            names.insert(&item.entry_name),
            "Duplicate download ZIP entry"
        );
    }
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        ".download-zip-{}.tmp",
        plain_libs::utils::uuid::Uuid::new_v4()
    ));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    let temporary = Temporary(path);
    let mut writer = plain_libs::zip::ZipWriter::new(file);
    let options = plain_libs::zip::write::SimpleFileOptions::default()
        .compression_method(plain_libs::zip::CompressionMethod::Deflated);
    let mut buffer = vec![0; 64 * 1024];
    for item in items {
        let metadata = std::fs::metadata(&item.source_path)?;
        if metadata.is_dir() {
            writer.add_directory(item.entry_name, options)?;
            continue;
        }
        let mut source = std::fs::File::open(item.source_path)?;
        ensure!(
            source.metadata()?.is_file(),
            "Download ZIP source is not a file"
        );
        writer.start_file(item.entry_name, options)?;
        loop {
            ensure!(!canceled.load(Ordering::SeqCst), "Download ZIP canceled");
            let read = source.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read])?;
        }
    }
    ensure!(!canceled.load(Ordering::SeqCst), "Download ZIP canceled");
    let mut file = writer.finish()?;
    file.sync_all()?;
    file.seek(SeekFrom::Start(0))?;
    Ok((file, temporary))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/downloads/archive.rs"]
mod tests;
