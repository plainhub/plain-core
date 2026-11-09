use crate::server::ServerState;
use plain_rs::features::services::files::{
    content::{self, Content, Options, Size},
    links,
    previews::Temporary,
};
use plain_rs::infrastructure::host::request_scope::NativeRequestScope;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::watch;

#[derive(Clone)]
pub struct NativeResources {
    state: ServerState,
}

impl NativeResources {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }

    pub async fn open(
        &self,
        registry: &NativeResourceRegistry,
        scope: &NativeRequestScope,
        reference: &str,
        thumbnail: Option<Size>,
    ) -> Result<ResourceInfo, String> {
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("resource owner closed".into()),
            _ = stop.wait_for(|stopped| *stopped) => Err("Rust core stopped".into()),
            result = async {
                let _permit = registry.prepare().await?;
                let kind = if thumbnail.is_some() { links::ReadKind::Thumbnail } else { links::ReadKind::Original };
                let file = links::resolve_native_reference(&self.state, reference, kind).await.map_err(|error| error.to_string())?;
                let content = content::read(&self.state, file, Options {
                    access_order: self.state.thumbnails.next_access_order(),
                    action: if thumbnail.is_some() { content::Action::Preview } else { content::Action::Download },
                    size: thumbnail,
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    starts_view: false,
                }).await.map_err(|error| error.to_string())?;
                if let Content::Thumbnail { bytes, name } = content {
                    let bytes = bytes.map_err(|error| error.to_string())?.ok_or("resource unavailable")?;
                    if bytes.len() as u64 > MAX_RESOURCE_RESPONSE_BYTES { return Err("resource response exceeds limit".into()); }
                    let mime = content::thumbnail_mime(&bytes).to_owned();
                    let file = self.state.remote_cache.stage_stream_in(&self.state.cache.root().join("native_resources"), bytes.len() as u64, futures_util::stream::once(async move { Ok(bytes) })).await?;
                    registry.open_staged(scope, file, name, mime, Some(self.state.stop.clone())).await
                } else {
                    registry.open_content(scope, content, Some(self.state.stop.clone())).await
                }
            } => result,
        }
    }

    pub async fn open_peer(
        &self,
        registry: &NativeResourceRegistry,
        scope: &NativeRequestScope,
        peer_id: &str,
        reference: &str,
        thumbnail: Option<Size>,
    ) -> Result<ResourceInfo, String> {
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("resource owner closed".into()),
            _ = stop.wait_for(|stopped| *stopped) => Err("Rust core stopped".into()),
            result = async {
                let _permit = registry.prepare().await?;
                let resource = crate::managed_peers::resource::PeerResource::open(self.state.clone(), peer_id, reference, thumbnail).await?;
                registry.open_peer(scope, reference, resource).await
            } => result,
        }
    }
}

mod export;
mod registry;
pub use export::ResourceDestination;
pub use registry::{NativeResource, NativeResourceRegistry, ResourceAccessError};

pub const MAX_RESOURCE_READ_BYTES: usize = 64 * 1024;
pub const MAX_RESOURCE_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Serialize)]
pub struct FilePreview {
    pub status: u16,
    pub headers: std::collections::BTreeMap<String, String>,
    pub content: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub inline_allowed: bool,
}

fn inline_allowed(mime: &str) -> bool {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    mime != "text/html" && !mime.ends_with("+xml") && !mime.ends_with("/xml")
}

struct Resource {
    source: Source,
    size: u64,
    closed: watch::Sender<bool>,
    stop: Option<watch::Receiver<bool>>,
}

enum Source {
    File { file: tokio::sync::Mutex<File> },
    Prepared(ResourceTemporary),
    Peer(crate::managed_peers::resource::PeerResource),
}

enum ResourceTemporary {
    Preview {
        _file: Temporary,
    },
    Staged {
        _file: plain_libs::temporary::TempPath,
    },
}

impl ResourceTemporary {
    fn path(&self) -> &Path {
        match self {
            Self::Preview { _file } => &_file.0,
            Self::Staged { _file } => _file.as_ref(),
        }
    }
}

async fn read_prepared(path: &Path, offset: u64, length: u64) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).await.map_err(|error| error.to_string())?;
    file.seek(std::io::SeekFrom::Start(offset))
        .await
        .map_err(|error| error.to_string())?;
    let mut bytes = vec![0; length as usize];
    for chunk in bytes.chunks_mut(MAX_RESOURCE_READ_BYTES) {
        file.read_exact(chunk)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

#[derive(Default)]
struct State {
    closed: bool,
    resources: HashMap<String, Arc<Resource>>,
}

#[derive(Default)]
pub struct NativeFileResources {
    state: Mutex<State>,
}

impl NativeFileResources {
    fn is_open(&self, id: &str) -> bool {
        let state = self.state.lock().unwrap();
        !state.closed
            && state.resources.get(id).is_some_and(|resource| {
                resource
                    .stop
                    .as_ref()
                    .is_none_or(|stop| !*stop.borrow() && stop.has_changed().is_ok())
                    && match &resource.source {
                        Source::Peer(peer) => peer.check_current().is_ok(),
                        _ => true,
                    }
            })
    }

    pub async fn open(&self, path: &Path) -> Result<ResourceInfo, String> {
        self.open_file(path, None, None, None, None).await
    }

    async fn open_file(
        &self,
        path: &Path,
        name: Option<String>,
        mime: Option<String>,
        temporary: Option<ResourceTemporary>,
        stop: Option<watch::Receiver<bool>>,
    ) -> Result<ResourceInfo, String> {
        if self.state.lock().unwrap().closed {
            return Err("resource owner closed".into());
        }
        let file = File::open(path).await.map_err(|error| error.to_string())?;
        let metadata = file.metadata().await.map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            return Err("resource must be a file".into());
        }
        let name = name.unwrap_or_else(|| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let mime = mime.unwrap_or_else(|| plain_libs::utils::mime::mime_from_ext(&name).into());
        self.insert(
            name,
            mime,
            metadata.len(),
            match temporary {
                Some(temporary) => {
                    drop(file);
                    Source::Prepared(temporary)
                }
                None => Source::File {
                    file: tokio::sync::Mutex::new(file),
                },
            },
            stop,
        )
    }

    async fn open_content(
        &self,
        content: Content,
        stop: Option<watch::Receiver<bool>>,
    ) -> Result<ResourceInfo, String> {
        match content {
            Content::File {
                path, name, mime, ..
            } => {
                self.open_file(&path, Some(name), Some(mime), None, stop)
                    .await
            }
            Content::Temporary { file, mime, name } => {
                let path = file.0.clone();
                self.open_file(
                    &path,
                    name,
                    Some(mime.into()),
                    Some(ResourceTemporary::Preview { _file: file }),
                    stop,
                )
                .await
            }
            _ => Err("resource unavailable".into()),
        }
    }

    async fn open_staged(
        &self,
        file: plain_libs::temporary::NamedTempFile,
        name: String,
        mime: String,
        stop: Option<watch::Receiver<bool>>,
    ) -> Result<ResourceInfo, String> {
        let path = file.path().to_path_buf();
        self.open_file(
            &path,
            Some(name),
            Some(mime),
            Some(ResourceTemporary::Staged {
                _file: file.into_temp_path(),
            }),
            stop,
        )
        .await
    }

    fn insert(
        &self,
        name: String,
        mime: String,
        size: u64,
        source: Source,
        stop: Option<watch::Receiver<bool>>,
    ) -> Result<ResourceInfo, String> {
        let info = ResourceInfo {
            id: plain_libs::utils::uuid::Uuid::new_v4().into_string(),
            inline_allowed: inline_allowed(&mime),
            mime,
            name,
            size,
        };
        let (closed, _) = watch::channel(false);
        let resource = Arc::new(Resource {
            source,
            size: info.size,
            closed,
            stop,
        });
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err("resource owner closed".into());
        }
        state.resources.insert(info.id.clone(), resource);
        Ok(info)
    }

    pub async fn read(&self, id: &str, offset: u64, length: usize) -> Result<Vec<u8>, String> {
        if length == 0 || length > MAX_RESOURCE_READ_BYTES {
            return Err("invalid resource read length".into());
        }
        let resource = self
            .state
            .lock()
            .unwrap()
            .resources
            .get(id)
            .cloned()
            .ok_or("resource unavailable")?;
        if offset > resource.size {
            return Err("resource offset exceeds size".into());
        }
        let length = (resource.size - offset).min(length as u64) as usize;
        let mut closed = resource.closed.subscribe();
        if *closed.borrow() || !self.is_open(id) {
            return Err("resource closed".into());
        }
        let stopped = async {
            if let Some(mut stop) = resource.stop.clone() {
                let _ = stop.wait_for(|stopped| *stopped).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let read = async {
            match &resource.source {
                Source::Prepared(file) => read_prepared(file.path(), offset, length as u64).await,
                Source::Peer(peer) => peer.read_range(offset, length as u64).await,
                Source::File { file, .. } => {
                    let mut file = file.lock().await;
                    file.seek(std::io::SeekFrom::Start(offset))
                        .await
                        .map_err(|error| error.to_string())?;
                    let mut bytes = vec![0; length];
                    file.read_exact(&mut bytes)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok::<Vec<u8>, String>(bytes)
                }
            }
        };
        let bytes = tokio::select! {
            _ = closed.changed() => return Err("resource closed".into()),
            _ = stopped => return Err("Rust core stopped".into()),
            result = read => result?,
        };
        if *closed.borrow() {
            return Err("resource closed".into());
        }
        Ok(bytes)
    }

    async fn read_range(&self, id: &str, offset: u64, length: u64) -> Result<Vec<u8>, String> {
        let resource = self
            .state
            .lock()
            .unwrap()
            .resources
            .get(id)
            .cloned()
            .ok_or("resource unavailable")?;
        let mut closed = resource.closed.subscribe();
        if *closed.borrow() || !self.is_open(id) {
            return Err("resource closed".into());
        }
        let stopped = async {
            if let Some(mut stop) = resource.stop.clone() {
                let _ = stop.wait_for(|stopped| *stopped).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let read = async {
            match &resource.source {
                Source::Peer(peer) => return peer.read_range(offset, length).await,
                Source::Prepared(file) => return read_prepared(file.path(), offset, length).await,
                _ => {}
            }
            let mut bytes = Vec::with_capacity(length as usize);
            while (bytes.len() as u64) < length {
                let count =
                    (length - bytes.len() as u64).min(MAX_RESOURCE_READ_BYTES as u64) as usize;
                let chunk = self.read(id, offset + bytes.len() as u64, count).await?;
                if chunk.is_empty() {
                    return Err("resource ended before the requested range".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        };
        tokio::select! {
            biased;
            _ = closed.wait_for(|closed| *closed) => Err("resource closed".into()),
            _ = stopped => Err("Rust core stopped".into()),
            result = read => result,
        }
    }

    pub fn close(&self, id: &str) {
        if let Some(resource) = self.state.lock().unwrap().resources.remove(id) {
            resource.closed.send_replace(true);
        }
    }

    pub fn close_all(&self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        for (_, resource) in state.resources.drain() {
            resource.closed.send_replace(true);
        }
    }
}

impl Drop for NativeFileResources {
    fn drop(&mut self) {
        self.close_all();
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/native/resources.rs"]
mod tests;
