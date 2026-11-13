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
        let cached = if let Some(url) = shared_resource(&source) {
            let thumbnail = url
                .query_pairs()
                .any(|(key, value)| key == "action" && value == "preview");
            Some(
                state
                    .remote_cache
                    .download(
                        &source,
                        &name,
                        thumbnail.then_some(source.as_str()),
                        state.stop.clone(),
                        Some(&state.lan.client),
                    )
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?,
            )
        } else {
            None
        };
        let temporary = if cached.is_some() {
            None
        } else if source.starts_with("content://") || source.starts_with("ph://") {
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
        let path = match (cached, temporary.as_ref()) {
            (Some(path), _) => path,
            (None, Some(file)) => file.path().to_string_lossy().into_owned(),
            (None, None) => crate::features::services::files::references::resolve(
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

fn shared_resource(raw: &str) -> Option<plain_libs::protocols::http_client::Url> {
    let url = plain_libs::protocols::http_client::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.path() != "/fs"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let id = url.query_pairs().find(|(key, _)| key == "id")?.1;
    if !matches!(
        crate::features::files::resource_link::parse(&id).ok()?.0,
        crate::features::files::resource_link::Scope::Share(_)
    ) {
        return None;
    }
    Some(url)
}
