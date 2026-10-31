use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub async fn cache(
    resources: &plain_rs::host::resources::NativeResources,
    directory: &Path,
    path: &str,
) -> Result<Option<PathBuf>, String> {
    let cover =
        plain_rs::services::files::input::read_source(resources, directory, path, |reader| {
            Ok(plain_rs::media::cover::extract_embedded_cover_reader(
                reader,
            ))
        })
        .await?;
    let Some(cover) = cover else { return Ok(None) };
    let directory = directory.join("audio-covers");
    tokio::task::spawn_blocking(move || publish(&directory, &cover.bytes))
        .await
        .map_err(|error| error.to_string())?
        .map(Some)
}

fn publish(directory: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    let path = directory.join(digest);
    if path.try_exists().map_err(|error| error.to_string())? {
        if std::fs::read(&path).map_err(|error| error.to_string())? == bytes {
            return Ok(path);
        }
    }
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(&path)
        .map_err(|error| error.to_string())?;
    Ok(path)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/audio/covers.rs"]
mod tests;
