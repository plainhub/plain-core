use plain_rs::host::resources::NativeResources;
use plain_rs::{db::DAppFile, files::app_files::FileStore};
use std::sync::Arc;
use tokio::sync::watch;

pub struct Import {
    pub source: String,
    pub name: String,
    pub mime: String,
    pub delete_source: bool,
}

pub async fn import(
    store: &Arc<FileStore>,
    resources: &NativeResources,
    cache: &std::path::Path,
    request: Import,
    stop: watch::Receiver<bool>,
    cache_changes: Arc<tokio::sync::RwLock<()>>,
) -> Result<DAppFile, String> {
    if *stop.borrow() {
        return Err("File import cancelled".into());
    }
    let platform = request.source.starts_with("content://") || request.source.starts_with("ph://");
    if platform && request.delete_source {
        return Err("Deleting an imported platform resource requires its platform adapter".into());
    }
    let cache_guard = cache_changes.read_owned().await;
    if *stop.borrow() {
        return Err("File import cancelled".into());
    }
    let temporary = if platform {
        Some(
            plain_rs::services::files::source_files::stage(
                &request.source,
                store.db.clone(),
                &store.directory,
                resources,
                cache,
                stop,
            )
            .await?,
        )
    } else {
        None
    };
    let source = match temporary.as_ref() {
        Some(file) => file.path().to_owned(),
        None => plain_rs::services::files::references::resolve(
            store.db.clone(),
            store.directory.clone(),
            &request.source,
        )
        .map_err(|error| error.to_string())?
        .into(),
    };
    let store = store.clone();
    tokio::task::spawn_blocking(move || {
        let _cache = cache_guard;
        let _temporary = temporary;
        store.import_file(&source, &request.name, &request.mime, request.delete_source)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/imports.rs"]
mod tests;
