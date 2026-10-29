use crate::assembly::server::ServerState;
use anyhow::Result;

pub(crate) async fn copy(
    state: &ServerState,
    caller: String,
    source: String,
    directory: String,
    name: String,
) -> Result<Option<String>> {
    super::file_tasks::named_directory(&directory, &name)?;
    let state = state.clone();
    tokio::spawn(async move {
        let _cache = state.cache_changes.read().await;
        anyhow::ensure!(!*state.stop.borrow(), "Rust core stopped");
        let temporary = if source.starts_with("content://") || source.starts_with("ph://") {
            Some(
                super::source_files::stage(
                    &source,
                    state.db.clone(),
                    &state.directory,
                    &state.bridge,
                    state.cache.root(),
                    state.stop.clone(),
                )
                .await
                .map_err(anyhow::Error::msg)?,
            )
        } else if source.starts_with("http://") || source.starts_with("https://") {
            Some(
                state
                    .remote_cache
                    .stage(&source, state.stop.clone())
                    .await
                    .map_err(anyhow::Error::msg)?,
            )
        } else {
            None
        };
        anyhow::ensure!(!*state.stop.borrow(), "Rust core stopped");
        let path = match temporary.as_ref() {
            Some(file) => file.path().to_string_lossy().into_owned(),
            None => super::references::resolve(state.db.clone(), state.directory.clone(), &source)?,
        };
        state
            .files
            .copy_to_directory(&caller, path, directory, name)
            .await
    })
    .await?
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/named_copy.rs"]
mod tests;
