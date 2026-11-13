mod archive;
pub mod chat;
mod io;
pub mod metadata;
pub mod resources;
mod runtime;
mod transfer;

use crate::features::downloads::{Download, Source};
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
pub use runtime::Runtime;
use std::path::Path;

pub fn target_directory(state: &CoreRuntime, target: &str) -> Result<String> {
    let path = if target.is_empty() {
        let root = &state.host.startup().downloads_dir;
        ensure!(!root.is_empty(), "Downloads directory unavailable");
        Path::new(root).join("PlainApp")
    } else {
        target.into()
    };
    ensure!(
        path.is_absolute() && !path.as_os_str().as_encoded_bytes().contains(&0),
        "Invalid download directory"
    );
    Ok(path
        .to_str()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid directory encoding"))?
        .into())
}

pub async fn create(
    state: &CoreRuntime,
    source: Source,
    target: Option<String>,
    zip_name: Option<String>,
) -> Result<Download> {
    let mut peers = vec![];
    let mut attachments = vec![];
    let (target, title) = match &source {
        Source::Resources { urls } => {
            ensure!(
                !urls.is_empty() && urls.len() <= 100_000,
                "Download resources required"
            );
            ensure!(
                urls.iter().collect::<std::collections::HashSet<_>>().len() == urls.len(),
                "Duplicate download resource"
            );
            for url in urls {
                resources::url(state, url)?;
            }
            peers = resources::access::prepare(state, urls)?;
            let target = target_directory(
                state,
                target
                    .as_deref()
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Download directory required"))?,
            )?;
            state.file_platform.authorize(&target, true).await?;
            if let Some(name) = &zip_name {
                transfer::validate_name(name)?;
            }
            (
                target,
                zip_name.clone().unwrap_or_else(|| "Download".into()),
            )
        }
        Source::Chat {
            message_id,
            peer_id,
            file_ids,
        } => {
            ensure!(
                target.is_none() && zip_name.is_none(),
                "Chat attachments use attachment storage"
            );
            ensure!(
                !file_ids.is_empty() && file_ids.len() <= 128,
                "Attachment selection required"
            );
            ensure!(
                file_ids.iter().all(|id| !id.is_empty())
                    && file_ids
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == file_ids.len(),
                "Invalid attachment selection"
            );
            let target = state.directory.join("files").to_string_lossy().into_owned();
            if let Some(task) = state.downloads.existing(&source, &target, None)? {
                return Ok(task);
            }
            peers.push(
                crate::infrastructure::db::chat_store::peers::get(&state.db, peer_id)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Peer unavailable"))?,
            );
            let files = chat::files(state, message_id, peer_id, file_ids, &Default::default())?;
            let title = files[0].name.clone();
            attachments = files;
            (
                state.directory.join("files").to_string_lossy().into_owned(),
                title,
            )
        }
    };
    state
        .downloads
        .create(source, target, zip_name, title, peers, attachments)
}

pub fn start(state: CoreRuntime) {
    runtime::start(state);
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/downloads/mod.rs"]
mod tests;
