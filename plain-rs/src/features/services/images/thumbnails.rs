use crate::runtime::Runtime as CoreRuntime;
mod queue;

pub(crate) use queue::Generation;
use queue::Queue;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tokio::sync::{Semaphore, oneshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[derive(Clone)]
pub struct Request {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub center_crop: bool,
    #[serde(rename = "mediaId")]
    pub file_id: String,
    pub file_name: String,
}
pub struct Thumbnails {
    pub(super) queue: Queue,
    dispatcher: Mutex<Option<tokio::task::JoinHandle<()>>>,
    requests: Semaphore,
    idle: tokio::sync::Notify,
    pub(super) warmer: Mutex<Option<tokio::task::JoinHandle<()>>>,
    #[cfg(feature = "media")]
    hot: plain_libs::media::thumbnails::lru::ThumbLru,
    #[cfg(any(test, feature = "test-seams"))]
    cache_reads: std::sync::atomic::AtomicUsize,
    outputs: Mutex<HashMap<String, oneshot::Sender<Vec<u8>>>>,
    #[cfg(any(test, feature = "test-seams"))]
    pub native_decodes: std::sync::atomic::AtomicUsize,
    #[cfg(any(test, feature = "test-seams"))]
    pub decode_gate: Mutex<Option<std::sync::Arc<Semaphore>>>,
}
impl Thumbnails {
    pub(crate) fn new() -> (Self, tokio::sync::mpsc::Receiver<Generation>) {
        let (queue, receiver) = Queue::new();
        (
            Self {
                queue,
                dispatcher: Mutex::new(None),
                requests: Semaphore::new(320),
                idle: tokio::sync::Notify::new(),
                warmer: Mutex::new(None),
                #[cfg(feature = "media")]
                hot: plain_libs::media::thumbnails::lru::ThumbLru::new(128 * 1024 * 1024, 4096),
                #[cfg(any(test, feature = "test-seams"))]
                cache_reads: std::sync::atomic::AtomicUsize::new(0),
                outputs: Mutex::new(HashMap::new()),
                #[cfg(any(test, feature = "test-seams"))]
                native_decodes: std::sync::atomic::AtomicUsize::new(0),
                #[cfg(any(test, feature = "test-seams"))]
                decode_gate: Mutex::new(None),
            },
            receiver,
        )
    }
}
async fn fingerprint(request: &Request) -> Result<Option<String>, String> {
    let request = request.clone();
    tokio::task::spawn_blocking(move || source_fingerprint(&request))
        .await
        .map_err(|error| error.to_string())?
}
fn source_fingerprint(request: &Request) -> Result<Option<String>, String> {
    let metadata = match std::fs::metadata(&request.path) {
        Ok(v) if v.is_file() => v,
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let system =
        crate::features::services::images::thumbnail_decoder::uses_system_thumbnail(request);
    let input = json!([
        request.path,
        metadata.len(),
        format!("{modified:?}"),
        request.width,
        request.height,
        request.center_crop,
        if system { &request.file_id } else { "" },
        if system { &request.file_name } else { "" },
        "thumbnail-v2"
    ])
    .to_string();
    Ok(Some(plain_libs::utils::hex::bytes_to_hex(&Sha256::digest(
        input.as_bytes(),
    ))))
}
fn validate_output(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err("invalid thumbnail output size".into());
    }
    Ok(())
}

async fn decode(
    state: &CoreRuntime,
    request: &Request,
    system: bool,
    cache: &Path,
) -> Result<Option<Vec<u8>>, String> {
    if !cfg!(any(target_os = "android", target_os = "ios")) {
        #[cfg(any(test, feature = "test-seams"))]
        state
            .thumbnails
            .native_decodes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        #[cfg(any(test, feature = "test-seams"))]
        {
            let gate = state.thumbnails.decode_gate.lock().unwrap().clone();
            if let Some(gate) = gate {
                gate.acquire()
                    .await
                    .map_err(|error| error.to_string())?
                    .forget();
            }
        }
        #[cfg(feature = "media")]
        let bytes = plain_libs::media::thumbnails::render(
            &request.path,
            request.width,
            request.height,
            request.center_crop,
            cache,
        )
        .await?;
        #[cfg(not(feature = "media"))]
        let bytes =
            crate::features::services::images::thumbnail_decoder::native(request, cache).await?;
        if let Some(bytes) = bytes.as_ref() {
            validate_output(bytes)?;
        }
        return Ok(bytes);
    }
    decode_sdk(state, request, system).await
}

async fn decode_sdk(
    state: &CoreRuntime,
    request: &Request,
    system: bool,
) -> Result<Option<Vec<u8>>, String> {
    let token = uuid::Uuid::new_v4().to_string();
    let (sender, mut receiver) = oneshot::channel();
    state
        .thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert(token.clone(), sender);
    let _output = Output {
        service: &state.thumbnails,
        token: token.clone(),
    };
    let mut stop = state.stop.clone();
    let result = tokio::select! {
        _ = stop.wait_for(|value| *value) => return Err("Rust core stopped".into()),
        result = state.host.call_wait("thumbnailDecode", json!({"outputToken":token,"path":request.path,"width":request.width,"height":request.height,"centerCrop":request.center_crop,"mediaId":if system { &request.file_id }else{""},"fileName":request.file_name})) => result?,
    };
    if result.is_null() {
        return Ok(None);
    }
    if result.as_bool() != Some(true) {
        return Err("invalid thumbnail receipt".into());
    }
    let bytes = receiver
        .try_recv()
        .map_err(|_| "missing thumbnail output")?;
    if bytes.is_empty() {
        return Err("empty thumbnail output".into());
    }
    Ok(Some(bytes))
}
struct Output<'a> {
    service: &'a Thumbnails,
    token: String,
}
impl Drop for Output<'_> {
    fn drop(&mut self) {
        self.service.outputs.lock().unwrap().remove(&self.token);
    }
}
impl Thumbnails {
    pub fn submit(&self, token: &str, bytes: Vec<u8>) -> Result<(), String> {
        validate_output(&bytes)?;
        self.outputs
            .lock()
            .unwrap()
            .remove(token)
            .ok_or("thumbnail output expired")?
            .send(bytes)
            .map_err(|_| "thumbnail output cancelled".into())
    }

    pub async fn get(
        &self,
        state: &CoreRuntime,
        request: Request,
    ) -> Result<Option<Vec<u8>>, Error> {
        let permit = self.requests.try_acquire().map_err(|_| Error::Busy)?;
        let _foreground = Foreground {
            permit: Some(permit),
            idle: &self.idle,
        };
        let mut stop = state.stop.clone();
        tokio::select! {
            _ = stop.wait_for(|value| *value) => Err(Error::Failed("Rust core stopped".into())),
            result = self.get_inner(state, request, false) => result.map_err(Error::Failed),
        }
    }

    pub(super) async fn get_inner(
        &self,
        state: &CoreRuntime,
        mut request: Request,
        background: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        if background {
            self.wait_for_foreground(state).await?;
        }
        if request.width == 0 || request.height == 0 || !Path::new(&request.path).is_absolute() {
            return Err("positive thumbnail dimensions and absolute path required".into());
        }
        let state_for_io = state.clone();
        let Some((request, cache, key, cached, bytes)) = tokio::task::spawn_blocking(move || {
            let canonical = match std::fs::canonicalize(&request.path) {
                Ok(path) => path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            request.path = canonical.to_string_lossy().into_owned();
            let cache = state_for_io
                .file_platform
                .thumbnail_cache(&request.path)
                .map_err(|error| error.to_string())?;
            let Some(key) = source_fingerprint(&request)? else {
                return Ok(None);
            };
            let cached = cache
                .join("thumbs")
                .join("rust")
                .join(format!("{key}.thumb"));
            let system =
                crate::features::services::images::thumbnail_decoder::uses_system_thumbnail(
                    &request,
                );
            let bytes = if system {
                None
            } else {
                state_for_io.thumbnails.cached_bytes(&cached)?
            };
            #[cfg(feature = "media")]
            let bytes = if !system && bytes.is_none() {
                plain_libs::media::thumbnails::read_original_thumbnail(
                    Path::new(&request.path),
                    request.width,
                    request.height,
                    request.center_crop,
                )
                .map_err(|error| error.to_string())?
            } else {
                bytes
            };
            if bytes.is_some() && source_fingerprint(&request)?.as_deref() != Some(&key) {
                return Ok(None);
            }
            Ok(Some((request, cache, key, cached, bytes)))
        })
        .await
        .map_err(|error| error.to_string())??
        else {
            return Ok(None);
        };
        if bytes.is_some() {
            return Ok(bytes);
        }
        self.queue.submit(request, cache, key, cached).await
    }

    async fn generate(state: &CoreRuntime, job: &Generation) -> Result<Option<Vec<u8>>, String> {
        let Generation {
            request,
            cache,
            key,
            cached,
        } = job;
        let system = super::thumbnail_decoder::uses_system_thumbnail(request);
        if system {
            if let Some(bytes) = decode(state, request, true, cache).await? {
                return Ok(
                    if fingerprint(request).await?.as_deref() == Some(key.as_str()) {
                        Some(bytes)
                    } else {
                        None
                    },
                );
            }
        }
        let cached_for_io = cached.clone();
        let thumbnails = state.thumbnails.clone();
        let bytes = tokio::task::spawn_blocking(move || thumbnails.cached_bytes(&cached_for_io))
            .await
            .map_err(|error| error.to_string())??;
        if let Some(bytes) = bytes {
            return Ok(
                if fingerprint(request).await?.as_deref() == Some(key.as_str()) {
                    Some(bytes)
                } else {
                    None
                },
            );
        }
        let Some(bytes) = decode(state, request, false, cache).await? else {
            return Ok(None);
        };
        if fingerprint(request).await?.as_deref() != Some(key.as_str()) {
            return Ok(None);
        }
        let stored = bytes.clone();
        let cached = cached.clone();
        let hot_key = cached.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            use std::io::Write;
            std::fs::create_dir_all(cached.parent().unwrap())?;
            let temporary = cached.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
            let _cleanup = Temporary(temporary.clone());
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&stored)?;
            file.flush()?;
            drop(file);
            std::fs::rename(&temporary, &cached)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
        #[cfg(feature = "media")]
        state
            .thumbnails
            .hot
            .put(hot_key, std::sync::Arc::new(bytes.clone()));
        #[cfg(not(feature = "media"))]
        let _ = hot_key;
        Ok(Some(bytes))
    }

    pub(super) async fn wait_for_foreground(&self, state: &CoreRuntime) -> Result<(), String> {
        while self.requests.available_permits() != 320 {
            let idle = self.idle.notified();
            tokio::pin!(idle);
            idle.as_mut().enable();
            if self.requests.available_permits() == 320 {
                break;
            }
            let mut stop = state.stop.clone();
            tokio::select! {
                _ = stop.wait_for(|value| *value) => return Err("Rust core stopped".into()),
                _ = idle => {},
            }
        }
        Ok(())
    }

    #[cfg(any(test, feature = "test-seams"))]
    pub fn admitted_requests(&self) -> usize {
        320 - self.requests.available_permits()
    }

    pub(crate) fn start(state: CoreRuntime, receiver: tokio::sync::mpsc::Receiver<Generation>) {
        let service = state.thumbnails.clone();
        *service.dispatcher.lock().unwrap() = Some(tokio::spawn(Queue::run(state, receiver)));
    }

    pub async fn shutdown(&self) {
        let task = self.warmer.lock().unwrap().take();
        if let Some(task) = task {
            task.abort();
            let _ = task.await;
        }
        let task = self.dispatcher.lock().unwrap().take();
        if let Some(task) = task {
            task.await.expect("thumbnail dispatcher panicked");
        }
    }
}
#[derive(Debug)]
pub enum Error {
    Busy,
    Failed(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("Thumbnail request capacity reached"),
            Self::Failed(error) => formatter.write_str(error),
        }
    }
}
impl std::error::Error for Error {}
struct Foreground<'a> {
    permit: Option<tokio::sync::SemaphorePermit<'a>>,
    idle: &'a tokio::sync::Notify,
}
impl Drop for Foreground<'_> {
    fn drop(&mut self) {
        drop(self.permit.take());
        self.idle.notify_waiters();
    }
}
impl Thumbnails {
    fn cached_bytes(&self, path: &PathBuf) -> Result<Option<Vec<u8>>, String> {
        #[cfg(feature = "media")]
        if let Some(bytes) = self.hot.get(path) {
            return Ok(Some(bytes.as_ref().clone()));
        }
        #[cfg(any(test, feature = "test-seams"))]
        self.cache_reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match std::fs::read(path) {
            Ok(bytes) if !bytes.is_empty() => {
                #[cfg(feature = "media")]
                self.hot
                    .put(path.clone(), std::sync::Arc::new(bytes.clone()));
                Ok(Some(bytes))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/images/thumbnails.rs"]
mod tests;
