use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{
    collections::HashSet,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    pub source_path: PathBuf,
    pub entry_name: String,
}
pub struct Temporary(pub PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub struct Cancel(pub Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
pub fn pack(
    directory: PathBuf,
    items: Vec<Item>,
    canceled: Arc<AtomicBool>,
) -> Result<(std::fs::File, Temporary)> {
    ensure!(
        !items.is_empty() && items.len() <= 100_000,
        "Invalid shared ZIP selection"
    );
    let mut names = HashSet::new();
    for item in &items {
        ensure!(item.source_path.is_absolute(), "Invalid shared ZIP source");
        ensure!(
            !item.entry_name.is_empty()
                && item.entry_name.split('/').all(|part| !part.is_empty()
                    && part != "."
                    && part != ".."
                    && !part.contains(['\\', '\0'])),
            "Invalid shared ZIP entry"
        );
        ensure!(names.insert(&item.entry_name), "Duplicate shared ZIP entry");
    }
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!(".share-zip-{}.tmp", uuid::Uuid::new_v4()));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    let temporary = Temporary(path);
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = vec![0; 64 * 1024];
    for item in items {
        let mut source = std::fs::File::open(item.source_path)?;
        ensure!(
            source.metadata()?.is_file(),
            "Shared ZIP source is not a file"
        );
        writer.start_file(item.entry_name, options)?;
        loop {
            ensure!(!canceled.load(Ordering::SeqCst), "Shared ZIP canceled");
            let read = source.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read])?;
        }
    }
    ensure!(!canceled.load(Ordering::SeqCst), "Shared ZIP canceled");
    let mut file = writer.finish()?;
    file.seek(SeekFrom::Start(0))?;
    Ok((file, temporary))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/shared_zip.rs"]
mod tests;
