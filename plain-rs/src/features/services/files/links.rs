use crate::features::files::resource_link::{self, ResourcePurpose, ResourceType, Scope};
use crate::runtime::Runtime as CoreRuntime;
use std::path::Path;

pub struct File {
    pub path: String,
    pub media_id: String,
    pub name: String,
}

pub fn download_url(id: &str) -> String {
    format!(
        "/fs?id={}&action=DOWNLOAD",
        plain_libs::utils::query::url_encode(id)
    )
}

pub(crate) fn key(state: &CoreRuntime, scope: &Scope) -> plain_libs::anyhow::Result<Vec<u8>> {
    let token = match scope {
        Scope::Session(id) if state.native_client_id().is_ok_and(|native| native == *id) => state.token.to_string(),
        Scope::Session(id) => {
            state
                .db
                .session_get(id)?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Session unavailable"))?
                .url_token
        }
        Scope::Share(id) => {
            crate::features::files::shares::Service::new(state.db.clone(), state.prefs.clone())
                .active(id, true)?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Share unavailable"))?
                .url_token
        }
    };
    let key = plain_libs::utils::base64::base64_decode_checked(&token)?;
    plain_libs::anyhow::ensure!(key.len() == 32, "Invalid resource key");
    Ok(key)
}

pub fn decrypt(
    state: &CoreRuntime,
    id: &str,
    purpose: ResourcePurpose,
) -> plain_libs::anyhow::Result<Vec<u8>> {
    let (scope, payload) = resource_link::parse(id)?;
    plain_libs::anyhow::ensure!(!matches!(scope, Scope::Share(_)), "Invalid request scope");
    resource_link::decrypt(&scope, purpose, &key(state, &scope)?, payload)
}

pub async fn resolve_native_reference(
    state: &CoreRuntime,
    reference: &str,
) -> plain_libs::anyhow::Result<File> {
    let client_id = state
        .native_client_id()
        .map_err(plain_libs::anyhow::Error::msg)?;
    plain_libs::anyhow::ensure!(!reference.is_empty(), "Empty resource reference");
    let file = resolve_resource(
        state,
        Scope::Session(client_id),
        resource_link::Resource::reference(reference),
    )
    .await?;
    let physical_path = file
        .path
        .split_once("!zip!/")
        .map_or(file.path.as_str(), |(archive, _)| archive);
    state.file_platform.authorize(physical_path, false).await?;
    state
        .native_client_id()
        .map_err(plain_libs::anyhow::Error::msg)?;
    Ok(file)
}

pub async fn resolve(state: &CoreRuntime, id: &str) -> plain_libs::anyhow::Result<File> {
    let owner = state.clone();
    let id = id.to_owned();
    let (scope, resource) = tokio::task::spawn_blocking(move || -> plain_libs::anyhow::Result<_> {
        let (scope, payload) = resource_link::parse(&id)?;
        let resource = resource_link::open(&scope, &key(&owner, &scope)?, payload)?;
        Ok((scope, resource))
    })
    .await??;
    resolve_resource(state, scope, resource).await
}

async fn resolve_resource(
    state: &CoreRuntime,
    scope: Scope,
    resource: resource_link::Resource,
) -> plain_libs::anyhow::Result<File> {
    let media = if resource.r#type == ResourceType::Media {
        state
            .file_platform
            .resource_media_file(&resource.id)
            .await?
    } else {
        File {
            path: String::new(),
            media_id: String::new(),
            name: String::new(),
        }
    };
    let state = state.clone();
    tokio::task::spawn_blocking(move || -> plain_libs::anyhow::Result<File> {
        let mut file = media;
        let authorize = matches!(
            resource.r#type,
            ResourceType::SharedFile
                | ResourceType::Media
                | ResourceType::File
        );
        match resource.r#type {
            ResourceType::SharedFile => {
                let Scope::Share(share_id) = scope else {
                    plain_libs::anyhow::bail!("Invalid share scope")
                };
                file.path = crate::features::files::shares::Service::new(
                    state.db.clone(),
                    state.prefs.clone(),
                )
                .resolve(&share_id, &resource.id, true)?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Shared file unavailable"))?;
            }
            ResourceType::PackageIcon => {
                plain_libs::anyhow::ensure!(
                    !resource.id.contains(['/', '\\', '\0']),
                    "Invalid package ID"
                );
                file.path = format!("pkgicon://{}", resource.id);
            }
            ResourceType::AppFile => {
                let row = state
                    .db
                    .app_file_get(resource.id.split('.').next().unwrap_or_default())?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Attachment unavailable"))?;
                let suffix = std::path::Path::new(&row.real_path)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid attachment path"))?;
                file.path = crate::features::files::app_files::FileStore::new(
                    state.db.clone(),
                    state.directory.clone(),
                )
                .resolve(suffix)
                .map_err(plain_libs::anyhow::Error::msg)?
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

pub fn authorize_path(state: &CoreRuntime, reference: &str) -> plain_libs::anyhow::Result<()> {
    if reference.starts_with("content://") || reference.starts_with("ph://") {
        return Ok(());
    }
    let path = reference
        .split_once("!zip!/")
        .map_or(reference, |(archive, _)| archive);
    let path = Path::new(path);
    let candidate = super::platform::paths::canonical(path)?;
    plain_libs::anyhow::ensure!(
        !state.file_platform.is_protected_file(path) && !state.file_platform.is_protected_file(&candidate),
        "Private application file"
    );
    let [directory, cache] = state
        .file_platform
        .private_roots
        .as_ref()
        .map_err(|error| plain_libs::anyhow::anyhow!(error.clone()))?;
    if candidate.starts_with(&cache) {
        let public = ["image_cache", "video_cache", "share_open", "remote_files"];
        plain_libs::anyhow::ensure!(
            public
                .iter()
                .any(|name| candidate.starts_with(cache.join(name))),
            "Private cache path"
        );
        return Ok(());
    }
    if candidate.starts_with(&directory) {
        let paths = crate::infrastructure::data_paths::DataPaths::new(&directory);
        if candidate == paths.pomodoro_sound() {
            return Ok(());
        }
        if candidate.starts_with(directory.join("files")) {
            let suffix = candidate
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid attachment path"))?;
            let owned = crate::features::files::app_files::FileStore::new(
                state.db.clone(),
                state.directory.clone(),
            )
            .resolve(suffix)
            .map_err(plain_libs::anyhow::Error::msg)?;
            plain_libs::anyhow::ensure!(
                owned.canonicalize()? == candidate,
                "Invalid attachment path"
            );
            return Ok(());
        }
        plain_libs::anyhow::bail!("Private application path");
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/links.rs"]
mod tests;
