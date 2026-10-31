use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::utils::keyed_locks::KeyedLocks;
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
    locks: KeyedLocks,
    capacity: Semaphore,
    outputs: Mutex<HashMap<String, oneshot::Sender<Vec<u8>>>>,
    #[cfg(any(test, feature = "test-seams"))]
    pub native_decodes: std::sync::atomic::AtomicUsize,
}
impl Default for Thumbnails {
    fn default() -> Self {
        Self {
            locks: KeyedLocks::new(),
            capacity: Semaphore::new(4),
            outputs: Mutex::new(HashMap::new()),
            #[cfg(any(test, feature = "test-seams"))]
            native_decodes: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
async fn fingerprint(request: &Request) -> Result<Option<String>, String> {
    let metadata = match tokio::fs::metadata(&request.path).await {
        Ok(v) if v.is_file() => v,
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let modified = metadata.modified().map_err(|e| e.to_string())?;
    let input = json!([
        request.path,
        metadata.len(),
        format!("{modified:?}"),
        request.width,
        request.height,
        request.center_crop,
        request.file_id,
        request.file_name,
        "mobile-v1"
    ])
    .to_string();
    Ok(Some(plain_rs::utils::hex::bytes_to_hex(&Sha256::digest(
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
        #[cfg(feature = "media")]
        let bytes = plain_rs::media::thumbnails::render(
            &request.path,
            request.width,
            request.height,
            request.center_crop,
            cache,
        )
        .await?;
        #[cfg(not(feature = "media"))]
        let bytes = plain_rs::services::images::thumbnail_decoder::native(request, cache).await?;
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
    let result=state.host.call_wait("thumbnailDecode",json!({"outputToken":token,"path":request.path,"width":request.width,"height":request.height,"centerCrop":request.center_crop,"mediaId":if system { &request.file_id }else{""},"fileName":request.file_name})).await?;
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
        mut request: Request,
    ) -> Result<Option<Vec<u8>>, String> {
        if request.width == 0 || request.height == 0 || !Path::new(&request.path).is_absolute() {
            return Err("positive thumbnail dimensions and absolute path required".into());
        }
        let canonical = match tokio::fs::canonicalize(&request.path).await {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        request.path = canonical.to_string_lossy().into_owned();
        let cache = state
            .file_platform
            .thumbnail_cache(&request.path)
            .await
            .map_err(|error| error.to_string())?;
        let Some(key) = fingerprint(&request).await? else {
            return Ok(None);
        };
        let cached = cache
            .join("thumbs")
            .join("rust")
            .join(format!("{key}.thumb"));
        let system = plain_rs::services::images::thumbnail_decoder::uses_system_thumbnail(&request);
        self.locks
            .with_lock(key.clone(), async {
                if !system {
                    match tokio::fs::read(&cached).await {
                        Ok(bytes) if !bytes.is_empty() => return Ok(Some(bytes)),
                        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                            return Err(e.to_string());
                        }
                        _ => {}
                    }
                }
                let _permit = self.capacity.acquire().await.map_err(|e| e.to_string())?;
                if system {
                    if let Some(bytes) = decode(state, &request, true, &cache).await? {
                        return Ok(if fingerprint(&request).await?.as_deref() == Some(&key) {
                            Some(bytes)
                        } else {
                            None
                        });
                    }
                }
                match tokio::fs::read(&cached).await {
                    Ok(bytes) if !bytes.is_empty() => return Ok(Some(bytes)),
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
                    _ => {}
                }
                let Some(bytes) = decode(state, &request, false, &cache).await? else {
                    return Ok(None);
                };
                if fingerprint(&request).await?.as_deref() != Some(&key) {
                    return Ok(None);
                }
                let stored = bytes.clone();
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
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;
                Ok(Some(bytes))
            })
            .await
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/images/thumbnails.rs"]
mod tests;
