use super::{MAX_RESOURCE_READ_BYTES, NativeRequestScope, NativeResources};
use futures_util::StreamExt;
use plain_rs::features::services::files::{
    content::{self, Content, Options},
    file_tasks::named_directory,
    links,
};
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

pub enum ResourceDestination {
    Directory { directory: String, name: String },
    File(PathBuf),
}

impl NativeResources {
    pub async fn export(
        &self,
        scope: &NativeRequestScope,
        peer_id: Option<&str>,
        reference: &str,
        destination: ResourceDestination,
    ) -> Result<String, String> {
        let (path, overwrite) = match destination {
            ResourceDestination::Directory { directory, name } => (
                named_directory(&directory, &name)
                    .map_err(|error| error.to_string())?
                    .join(name),
                false,
            ),
            ResourceDestination::File(path) => (path, true),
        };
        if !path.is_absolute() {
            return Err("Absolute destination required".into());
        }
        let directory = path.parent().ok_or("Destination directory required")?;
        let mut stop = self.state.stop.clone();
        let prepare = async {
            self.state.native_client_id()?;
            self.state
                .file_platform
                .authorize(path.to_str().ok_or("Invalid destination")?, true)
                .await
                .map_err(|error| error.to_string())?;
            if tokio::fs::metadata(&path)
                .await
                .is_ok_and(|metadata| !metadata.is_file())
            {
                return Err("Regular destination file required".into());
            }
            if let Some(peer_id) = peer_id {
                let peer = crate::managed_peers::resource::PeerResource::open(
                    self.state.clone(),
                    peer_id,
                    reference,
                    None,
                )
                .await?;
                let response = peer.download().await?;
                let stream = response.bytes_stream().map(|chunk| {
                    peer.check_current()?;
                    chunk.map_err(|error| error.to_string())
                });
                let file = self
                    .state
                    .remote_cache
                    .stage_stream_in(directory, peer.size, stream)
                    .await?;
                peer.check_current()?;
                return Ok((Some(file), Some(peer)));
            }
            let source =
                links::resolve_native_reference(&self.state, reference, links::ReadKind::Original)
                    .await
                    .map_err(|error| error.to_string())?;
            let content = content::read(
                &self.state,
                source,
                Options {
                    access_order: self.state.thumbnails.next_access_order(),
                    action: content::Action::Download,
                    size: None,
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    starts_view: false,
                },
            )
            .await
            .map_err(|error| error.to_string())?;
            let (source, _temporary) = match content {
                Content::File { path, .. } => (path, None),
                Content::Temporary { file, .. } => (file.0.clone(), Some(file)),
                _ => return Err("File export unavailable".into()),
            };
            if overwrite
                && path.exists()
                && plain_libs::utils::file_identity::same(&source, &path)
                    .map_err(|error| error.to_string())?
            {
                return Ok((None, None));
            }
            let input = tokio::fs::File::open(&source)
                .await
                .map_err(|error| error.to_string())?;
            let metadata = input.metadata().await.map_err(|error| error.to_string())?;
            if !metadata.is_file() {
                return Err("Regular source file required".into());
            }
            let size = metadata.len();
            let stream = futures_util::stream::try_unfold(
                (input, size),
                |(mut input, remaining)| async move {
                    if remaining == 0 {
                        return Ok::<_, String>(None);
                    }
                    let mut bytes = vec![0; remaining.min(MAX_RESOURCE_READ_BYTES as u64) as usize];
                    let count = input
                        .read(&mut bytes)
                        .await
                        .map_err(|error| error.to_string())?;
                    if count == 0 {
                        return Err("Resource ended before declared length".into());
                    }
                    bytes.truncate(count);
                    Ok(Some((
                        plain_libs::axum::body::Bytes::from(bytes),
                        (input, remaining - count as u64),
                    )))
                },
            );
            self.state
                .remote_cache
                .stage_stream_in(directory, size, stream)
                .await
                .map(|file| (Some(file), None))
        };
        let (temporary, peer) = tokio::select! {
            biased;
            _ = scope.wait_closed() => return Err("resource owner closed".into()),
            _ = stop.wait_for(|stopped| *stopped) => return Err("Rust core stopped".into()),
            result = prepare => result?,
        };
        self.state.native_client_id()?;
        if scope.is_closed() {
            return Err("resource owner closed".into());
        }
        let Some(mut temporary) = temporary else {
            return Ok(path.to_string_lossy().into_owned());
        };
        let state = self.state.clone();
        let scope = scope.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                state.native_client_id()?;
                if scope.is_closed() {
                    return Err("resource owner closed".into());
                }
                if let Some(peer) = &peer {
                    peer.check_current()?;
                }
                let destination = if overwrite {
                    path.clone()
                } else {
                    plain_libs::utils::unique_path::unique_sibling(&path)
                        .map_err(|error| error.to_string())?
                };
                let result = if overwrite {
                    temporary.persist(&destination)
                } else {
                    temporary.persist_noclobber(&destination)
                };
                match result {
                    Ok(_) => return Ok(destination.to_string_lossy().into_owned()),
                    Err(error)
                        if !overwrite
                            && error.error.kind() == std::io::ErrorKind::AlreadyExists =>
                    {
                        temporary = error.file
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
        })
        .await
        .map_err(|error| error.to_string())?
    }
}
