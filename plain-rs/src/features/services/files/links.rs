use crate::features::files::resource_link::{self, ResourceType, Scope};
use crate::features::media::catalog_paths::requires_foreground_scan;
use crate::runtime::Runtime as CoreRuntime;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadKind {
    Original,
    Thumbnail,
}

impl ReadKind {
    fn check(self, path: &Path) -> plain_libs::anyhow::Result<()> {
        plain_libs::anyhow::ensure!(
            self != Self::Thumbnail || !requires_foreground_scan(path),
            "Thumbnail requires explicit file access"
        );
        Ok(())
    }
}

pub struct File {
    pub path: String,
    pub media_id: String,
    pub name: String,
}

pub(crate) fn key(state: &CoreRuntime, scope: &Scope) -> plain_libs::anyhow::Result<Vec<u8>> {
    let token = match scope {
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

pub fn decrypt(state: &CoreRuntime, id: &str, purpose: &str) -> plain_libs::anyhow::Result<Vec<u8>> {
    let (scope, payload) = resource_link::parse(id)?;
    plain_libs::anyhow::ensure!(!matches!(scope, Scope::Share(_)), "Invalid request scope");
    resource_link::decrypt(&scope, purpose, &key(state, &scope)?, payload)
}

pub async fn resolve_native_reference(
    state: &CoreRuntime,
    reference: &str,
    kind: ReadKind,
) -> plain_libs::anyhow::Result<File> {
    let client_id = state
        .native_client_id()
        .map_err(plain_libs::anyhow::Error::msg)?;
    plain_libs::anyhow::ensure!(!reference.is_empty(), "Empty resource reference");
    let file = resolve_resource(
        state,
        Scope::Session(client_id),
        resource_link::Resource::reference(reference),
        kind,
    )
    .await?;
    let physical_path = file
        .path
        .split_once("!zip!/")
        .map_or(file.path.as_str(), |(archive, _)| archive);
    state.file_platform.authorize(physical_path, false).await?;
    state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?;
    Ok(file)
}

pub async fn resolve(
    state: &CoreRuntime,
    id: &str,
    kind: ReadKind,
) -> plain_libs::anyhow::Result<File> {
    let owner = state.clone();
    let id = id.to_owned();
    let (scope, resource) = tokio::task::spawn_blocking(move || -> plain_libs::anyhow::Result<_> {
        let (scope, payload) = resource_link::parse(&id)?;
        let resource = resource_link::open(&scope, &key(&owner, &scope)?, payload)?;
        if resource.r#type == ResourceType::ForegroundMediaThumbnail {
            plain_libs::anyhow::ensure!(
                matches!(&scope, Scope::Session(id) if &crate::features::services::sessions::native(&owner)?.client_id == id),
                "Foreground thumbnails require native session"
            );
        }
        Ok((scope, resource))
    })
    .await??;
    resolve_resource(state, scope, resource, kind).await
}

async fn resolve_resource(
    state: &CoreRuntime,
    scope: Scope,
    resource: resource_link::Resource,
    kind: ReadKind,
) -> plain_libs::anyhow::Result<File> {
    let kind = if resource.r#type == ResourceType::ForegroundMediaThumbnail {
        plain_libs::anyhow::ensure!(
            kind == ReadKind::Thumbnail,
            "Foreground resource requires thumbnail size"
        );
        ReadKind::Original
    } else {
        kind
    };
    let media_path = if matches!(
        resource.r#type,
        ResourceType::Media | ResourceType::ForegroundMediaThumbnail
    ) {
        state
            .file_platform
            .resource_media_path(&resource.id)
            .await?
    } else {
        String::new()
    };
    let state = state.clone();
    tokio::task::spawn_blocking(move || -> plain_libs::anyhow::Result<File> {
        let mut file = File {
            path: media_path,
            media_id: String::new(),
            name: String::new(),
        };
        let authorize = matches!(
            resource.r#type,
            ResourceType::SharedFile
                | ResourceType::Media
                | ResourceType::ForegroundMediaThumbnail
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
            ResourceType::Media | ResourceType::ForegroundMediaThumbnail => {
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
            authorize_path_for_read(&state, &file.path, kind)?;
        } else {
            kind.check(Path::new(&file.path))?;
        }
        Ok(file)
    })
    .await?
}

pub fn authorize_path(state: &CoreRuntime, reference: &str) -> plain_libs::anyhow::Result<()> {
    authorize_path_for_read(state, reference, ReadKind::Original)
}

fn authorize_path_for_read(
    state: &CoreRuntime,
    reference: &str,
    kind: ReadKind,
) -> plain_libs::anyhow::Result<()> {
    if reference.starts_with("content://") || reference.starts_with("ph://") {
        return Ok(());
    }
    let path = reference
        .split_once("!zip!/")
        .map_or(reference, |(archive, _)| archive);
    let path = Path::new(path);
    kind.check(path)?;
    let candidate = super::platform::paths::canonical(path)?;
    kind.check(&candidate)?;
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
        if candidate.starts_with(paths.note_images()) || candidate == paths.pomodoro_sound() {
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
