use crate::runtime::Runtime as CoreRuntime;
mod queue;

use bytes::Bytes;
pub(crate) use queue::Generation;
use queue::Queue;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
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
    #[serde(skip)]
    pub access_order: Option<u64>,
}
pub(crate) const THUMBNAIL_SIZE: u32 = 256;
const LOOKUP_LIMIT: usize = 320;

pub struct Thumbnails {
    pub(super) queue: Queue,
    dispatcher: Mutex<Option<tokio::task::JoinHandle<()>>>,
    lookups: Arc<Semaphore>,
    next_access: std::sync::atomic::AtomicU64,
    #[cfg(any(test, feature = "test-seams"))]
    pub(super) cache_bytes_written: std::sync::atomic::AtomicU64,
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
    pub(crate) fn new() -> Self {
        #[cfg(feature = "media")]
        plain_libs::media::thumbnails::admission::initialize_with_memory(|| {
            #[cfg(not(target_os = "ios"))]
            {
                let mut system = sysinfo::System::new();
                system.refresh_memory();
                system.total_memory()
            }
            #[cfg(target_os = "ios")]
            {
                plain_libs::platform::apple_device::total_memory()
                    .unwrap_or(0)
                    .max(0) as u64
            }
        });
        Self {
            queue: Queue::default(),
            dispatcher: Mutex::new(None),
            lookups: Arc::new(Semaphore::new(LOOKUP_LIMIT)),
            next_access: std::sync::atomic::AtomicU64::new(0),
            #[cfg(any(test, feature = "test-seams"))]
            cache_bytes_written: std::sync::atomic::AtomicU64::new(0),
            #[cfg(feature = "media")]
            hot: plain_libs::media::thumbnails::lru::ThumbLru::new(128 * 1024 * 1024, 4096),
            #[cfg(any(test, feature = "test-seams"))]
            cache_reads: std::sync::atomic::AtomicUsize::new(0),
            outputs: Mutex::new(HashMap::new()),
            #[cfg(any(test, feature = "test-seams"))]
            native_decodes: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(any(test, feature = "test-seams"))]
            decode_gate: Mutex::new(None),
        }
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
) -> Result<Option<Bytes>, String> {
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
        let bytes = crate::features::services::images::thumbnail_decoder::native(request, cache)
            .await?
            .map(Bytes::from);
        if let Some(bytes) = bytes.as_ref() {
            validate_output(bytes)?;
        }
        return Ok(bytes);
    }
    decode_sdk(state, request, system)
        .await
        .map(|bytes| bytes.map(Bytes::from))
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
    pub(super) fn workers(&self) -> usize {
        #[cfg(feature = "media")]
        {
            plain_libs::media::thumbnails::admission::global().workers()
        }
        #[cfg(not(feature = "media"))]
        {
            plain_libs::media::thumbnail_limits::workers(
                std::thread::available_parallelism().map_or(1, |n| n.get()),
            )
        }
    }

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

    pub fn next_access_order(&self) -> u64 {
        self.next_access
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn get(
        &self,
        state: &CoreRuntime,
        mut request: Request,
    ) -> Result<Option<Bytes>, Error> {
        let access_order = request
            .access_order
            .take()
            .unwrap_or_else(|| self.next_access_order());
        let mut stop = state.stop.clone();
        tokio::select! {
            _ = stop.wait_for(|value| *value) => Err(Error("Rust core stopped".into())),
            result = self.get_inner(state, request, access_order) => result.map_err(Error),
        }
    }

    async fn get_inner(
        &self,
        state: &CoreRuntime,
        mut request: Request,
        access_order: u64,
    ) -> Result<Option<Bytes>, String> {
        if request.width == 0 || request.height == 0 || !Path::new(&request.path).is_absolute() {
            return Err("positive thumbnail dimensions and absolute path required".into());
        }
        if request.width < 1024 && request.height < 1024 {
            request.width = THUMBNAIL_SIZE;
            request.height = THUMBNAIL_SIZE;
        }
        let lookup = self
            .lookups
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| error.to_string())?;
        let state_for_io = state.clone();
        let Some((request, cache, key, cached, bytes)) = tokio::task::spawn_blocking(move || {
            let _lookup = lookup;
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
            let cached = cache.join("thumbs").join("rust").join(format!(
                "{key}.{}",
                if plain_libs::utils::mime::mime_from_ext(&request.path) == "image/png" {
                    "png"
                } else {
                    "jpg"
                }
            ));
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
                .map(Bytes::from)
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
        self.queue
            .submit(
                Generation {
                    request,
                    cache,
                    key,
                    cached,
                },
                access_order,
            )
            .await
    }

    async fn generate(state: &CoreRuntime, job: &Generation) -> Result<Option<Bytes>, String> {
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
        let hot_key = cached.clone();
        let cached = cached.with_extension(if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "png"
        } else {
            "jpg"
        });
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
        #[cfg(any(test, feature = "test-seams"))]
        state
            .thumbnails
            .cache_bytes_written
            .fetch_add(bytes.len() as u64, std::sync::atomic::Ordering::Relaxed);
        #[cfg(feature = "media")]
        state.thumbnails.hot.put(hot_key, bytes.clone());
        #[cfg(not(feature = "media"))]
        let _ = hot_key;
        Ok(Some(bytes))
    }

    #[cfg(any(test, feature = "test-seams"))]
    pub fn pending_requests(&self) -> usize {
        self.queue.waiting()
    }

    pub(crate) fn start(state: CoreRuntime) {
        let service = state.thumbnails.clone();
        *service.dispatcher.lock().unwrap() = Some(tokio::spawn(Queue::run(state)));
    }

    pub async fn shutdown(&self) {
        let task = self.dispatcher.lock().unwrap().take();
        if let Some(task) = task {
            task.await.expect("thumbnail dispatcher panicked");
        }
    }
}
#[derive(Debug)]
pub struct Error(String);
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl Thumbnails {
    fn cached_bytes(&self, path: &PathBuf) -> Result<Option<Bytes>, String> {
        #[cfg(feature = "media")]
        if let Some(bytes) = self.hot.get(path) {
            return Ok(Some(bytes));
        }
        let alternate =
            path.with_extension(if path.extension().is_some_and(|value| value == "png") {
                "jpg"
            } else {
                "png"
            });
        for candidate in [path, &alternate] {
            #[cfg(any(test, feature = "test-seams"))]
            self.cache_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match std::fs::read(candidate) {
                Ok(bytes) if !bytes.is_empty() => {
                    let bytes = Bytes::from(bytes);
                    #[cfg(feature = "media")]
                    self.hot.put(path.clone(), bytes.clone());
                    return Ok(Some(bytes));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(None)
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
