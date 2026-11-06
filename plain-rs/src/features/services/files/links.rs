use crate::features::files::resource_link::{self, ResourceType, Scope};
use crate::runtime::Runtime as CoreRuntime;

pub struct File {
    pub path: String,
    pub media_id: String,
    pub name: String,
}

pub(crate) fn key(state: &CoreRuntime, scope: &Scope) -> anyhow::Result<Vec<u8>> {
    let token = match scope {
        Scope::Session(id) => {
            state
                .db
                .session_get(id)?
                .ok_or_else(|| anyhow::anyhow!("Session unavailable"))?
                .url_token
        }
        Scope::Global => state
            .prefs
            .get::<String>(crate::features::prefs::pref_keys::URL_TOKEN)?
            .ok_or_else(|| anyhow::anyhow!("URL token unavailable"))?,
        Scope::Share(id) => {
            crate::features::files::shares::Service::new(state.db.clone(), state.prefs.clone())
                .active(id, true)?
                .ok_or_else(|| anyhow::anyhow!("Share unavailable"))?
                .url_token
        }
    };
    let key = plain_libs::utils::base64::base64_decode_checked(&token)?;
    anyhow::ensure!(key.len() == 32, "Invalid resource key");
    Ok(key)
}

pub fn decrypt(state: &CoreRuntime, id: &str, purpose: &str) -> anyhow::Result<Vec<u8>> {
    let (scope, payload) = resource_link::parse(id)?;
    anyhow::ensure!(!matches!(scope, Scope::Share(_)), "Invalid request scope");
    resource_link::decrypt(&scope, purpose, &key(state, &scope)?, payload)
}

pub async fn resolve(state: &CoreRuntime, id: &str) -> anyhow::Result<File> {
    let owner = state.clone();
    let id = id.to_owned();
    let (scope, resource) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let (scope, payload) = resource_link::parse(&id)?;
        let resource = resource_link::open(&scope, &key(&owner, &scope)?, payload)?;
        Ok((scope, resource))
    })
    .await??;
    let media_path = if matches!(resource.r#type, ResourceType::Media) {
        state
            .file_platform
            .resource_media_path(&resource.id)
            .await?
    } else {
        String::new()
    };
    let state = state.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<File> {
        let mut file = File {
            path: media_path,
            media_id: String::new(),
            name: String::new(),
        };
        let authorize = matches!(
            resource.r#type,
            ResourceType::SharedFile | ResourceType::Media | ResourceType::File
        );
        match resource.r#type {
            ResourceType::SharedFile => {
                let Scope::Share(share_id) = scope else {
                    anyhow::bail!("Invalid share scope")
                };
                file.path = crate::features::files::shares::Service::new(
                    state.db.clone(),
                    state.prefs.clone(),
                )
                .resolve(&share_id, &resource.id, true)?
                .ok_or_else(|| anyhow::anyhow!("Shared file unavailable"))?;
            }
            ResourceType::PackageIcon => {
                anyhow::ensure!(
                    !resource.id.contains(['/', '\\', '\0']),
                    "Invalid package ID"
                );
                file.path = format!("pkgicon://{}", resource.id);
            }
            ResourceType::AppFile => {
                let row = state
                    .db
                    .app_file_get(resource.id.split('.').next().unwrap_or_default())?
                    .ok_or_else(|| anyhow::anyhow!("Attachment unavailable"))?;
                let suffix = std::path::Path::new(&row.real_path)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid attachment path"))?;
                file.path = crate::features::files::app_files::FileStore::new(
                    state.db.clone(),
                    state.directory.clone(),
                )
                .resolve(suffix)
                .map_err(anyhow::Error::msg)?
                .to_string_lossy()
                .into_owned();
                let names = crate::features::chat::attachments::app_file_store::file_name_map(
                    &state.db.get_all_chats(),
                );
                file.name =
                    crate::features::chat::attachments::app_file_store::display_name(&row, &names);
            }
            ResourceType::Media => {
                file.media_id = resource.id;
            }
            ResourceType::File => {
                file.path = super::references::resolve(
                    state.db.clone(),
                    state.directory.clone(),
                    &resource.id,
                )?;
            }
        }
        if authorize {
            authorize_path(&state, &file.path)?;
        }
        Ok(file)
    })
    .await?
}

pub fn authorize_path(state: &CoreRuntime, reference: &str) -> anyhow::Result<()> {
    if reference.starts_with("content://") || reference.starts_with("ph://") {
        return Ok(());
    }
    let path = reference
        .split_once("!zip!/")
        .map_or(reference, |(archive, _)| archive);
    let candidate = super::platform::paths::canonical(std::path::Path::new(path))?;
    let [directory, cache] = state
        .file_platform
        .private_roots
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))?;
    if candidate.starts_with(&cache) {
        let public = ["image_cache", "video_cache", "share_open", "remote_files"];
        anyhow::ensure!(
            public
                .iter()
                .any(|name| candidate.starts_with(cache.join(name))),
            "Private cache path"
        );
        return Ok(());
    }
    if candidate.starts_with(&directory) {
        let paths = crate::infrastructure::data_paths::DataPaths::new(&directory);
        if candidate.starts_with(paths.note_images()) || candidate == paths.pomodoro_sound() {
            return Ok(());
        }
        if candidate.starts_with(directory.join("files")) {
            let suffix = candidate
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid attachment path"))?;
            let owned = crate::features::files::app_files::FileStore::new(
                state.db.clone(),
                state.directory.clone(),
            )
            .resolve(suffix)
            .map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                owned.canonicalize()? == candidate,
                "Invalid attachment path"
            );
            return Ok(());
        }
        anyhow::bail!("Private application path");
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/links.rs"]
mod tests;
