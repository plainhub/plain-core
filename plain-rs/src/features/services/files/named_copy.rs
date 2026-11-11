use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::Result;

pub async fn copy(
    state: &CoreRuntime,
    caller: String,
    source: String,
    directory: String,
    name: String,
) -> Result<Option<String>> {
    crate::features::services::files::file_tasks::named_directory(&directory, &name)?;
    let state = state.clone();
    tokio::spawn(async move {
        let _cache = state.cache_changes.read().await;
        plain_libs::anyhow::ensure!(!*state.stop.borrow(), "Rust core stopped");
        let temporary = if source.starts_with("content://") || source.starts_with("ph://") {
            Some(
                crate::features::services::files::source_files::stage(
                    &source,
                    state.db.clone(),
                    &state.directory,
                    &state.resources,
                    state.cache.root(),
                    state.stop.clone(),
                )
                .await
                .map_err(plain_libs::anyhow::Error::msg)?,
            )
        } else if source.starts_with("http://") || source.starts_with("https://") {
            Some(
                state
                    .remote_cache
                    .stage(&source, state.stop.clone())
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?,
            )
        } else {
            None
        };
        plain_libs::anyhow::ensure!(!*state.stop.borrow(), "Rust core stopped");
        let path = match temporary.as_ref() {
            Some(file) => file.path().to_string_lossy().into_owned(),
            None => crate::features::services::files::references::resolve(
                state.db.clone(),
                state.directory.clone(),
                &source,
            )?,
        };
        state
            .files
            .copy_to_directory(&caller, path, directory, name)
            .await
    })
    .await?
}
