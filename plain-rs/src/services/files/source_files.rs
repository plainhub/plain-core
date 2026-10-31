use plain_rs::db::Db;
use plain_rs::host::resources::NativeResources;
use std::{path::Path, sync::Arc};
use tokio::{io::AsyncWriteExt, sync::watch};

pub async fn stage(
    source: &str,
    db: Arc<Db>,
    directory: &Path,
    resources: &NativeResources,
    parent: &Path,
    mut stop: watch::Receiver<bool>,
) -> Result<tempfile::NamedTempFile, String> {
    if *stop.borrow() {
        return Err("File input cancelled".into());
    }
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    let mut output =
        tokio::fs::File::from_std(temporary.reopen().map_err(|error| error.to_string())?);
    let mut cancelled = stop.clone();
    let copy = async {
        if source.starts_with("content://") || source.starts_with("ph://") {
            let mut input = resources
                .read_native(source, stop.clone())
                .await
                .map_err(|error| error.to_string())?;
            while let Some(bytes) = input.read().await? {
                output
                    .write_all(&bytes)
                    .await
                    .map_err(|error| error.to_string())?;
            }
        } else {
            let path =
                plain_rs::services::files::references::resolve(db, directory.to_path_buf(), source)
                    .map_err(|error| error.to_string())?;
            let mut input = tokio::fs::File::open(path)
                .await
                .map_err(|error| error.to_string())?;
            if !input
                .metadata()
                .await
                .map_err(|error| error.to_string())?
                .is_file()
            {
                return Err("Input must be a regular file".to_owned());
            }
            tokio::io::copy(&mut input, &mut output)
                .await
                .map_err(|error| error.to_string())?;
        }
        output.flush().await.map_err(|error| error.to_string())?;
        output.sync_all().await.map_err(|error| error.to_string())
    };
    tokio::select! {
        biased;
        _ = cancelled.changed() => return Err("File input cancelled".into()),
        result = copy => result?,
    }
    drop(output);
    if *stop.borrow_and_update() {
        return Err("File input cancelled".into());
    }
    Ok(temporary)
}
