use futures_util::{Stream, StreamExt};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    sync::{Semaphore, watch},
};

pub struct RemoteCache {
    cache: PathBuf,
    client: OnceLock<Result<reqwest::Client, String>>,
    verified_client: OnceLock<Result<reqwest::Client, String>>,
    locks: plain_rs::utils::keyed_locks::KeyedLocks,
    permits: Semaphore,
}
impl RemoteCache {
    pub fn new(cache: PathBuf) -> Self {
        Self {
            cache,
            client: OnceLock::new(),
            verified_client: OnceLock::new(),
            locks: plain_rs::utils::keyed_locks::KeyedLocks::new(),
            permits: Semaphore::new(4),
        }
    }
    fn directory(&self) -> PathBuf {
        self.cache.join("remote_files")
    }
    pub async fn download(
        &self,
        raw: &str,
        name: &str,
        cache_key: Option<&str>,
        mut stop: watch::Receiver<bool>,
        client: Option<&reqwest::Client>,
    ) -> Result<String, String> {
        if *stop.borrow() {
            return Err("Download cancelled".into());
        }
        let work = async {
            let url = url(raw)?;
            let directory = self.directory();
            self.download_with(&directory, raw, name, cache_key, || async move {
                let client = match client {
                    Some(client) => client,
                    None => self
                        .client
                        .get_or_init(|| {
                            reqwest::Client::builder()
                                .use_rustls_tls()
                                .danger_accept_invalid_certs(true)
                                .connect_timeout(Duration::from_secs(10))
                                .build()
                                .map_err(|error| error.to_string())
                        })
                        .as_ref()
                        .map_err(Clone::clone)?,
                };
                let response =
                    tokio::time::timeout(Duration::from_secs(30), client.get(url).send())
                        .await
                        .map_err(|_| "Download response timed out".to_owned())?
                        .map_err(|error| error.to_string())?;
                if !response.status().is_success() {
                    return Err(format!("Download HTTP {}", response.status().as_u16()));
                }
                let length = response.content_length();
                Ok((
                    length,
                    response
                        .bytes_stream()
                        .map(|chunk| chunk.map_err(|error| error.to_string())),
                ))
            })
            .await
        };
        tokio::select! {
            biased;
            _ = stop.changed() => Err("Download cancelled".into()),
            result = work => result,
        }
    }
    pub async fn stage(
        &self,
        raw: &str,
        mut stop: watch::Receiver<bool>,
    ) -> Result<tempfile::NamedTempFile, String> {
        if *stop.borrow() {
            return Err("Download cancelled".into());
        }
        let url = url(raw)?;
        let work = async {
            let _permit = self
                .permits
                .acquire()
                .await
                .map_err(|error| error.to_string())?;
            let client = self
                .verified_client
                .get_or_init(|| {
                    reqwest::Client::builder()
                        .use_rustls_tls()
                        .connect_timeout(Duration::from_secs(10))
                        .read_timeout(Duration::from_secs(10))
                        .redirect(reqwest::redirect::Policy::limited(20))
                        .build()
                        .map_err(|error| error.to_string())
                })
                .as_ref()
                .map_err(Clone::clone)?;
            let response = client
                .get(url)
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if !response.status().is_success() {
                return Err(format!("Download HTTP {}", response.status().as_u16()));
            }
            let expected = response.content_length();
            write_download(
                &self.directory(),
                expected,
                response
                    .bytes_stream()
                    .map(|chunk| chunk.map_err(|error| error.to_string())),
            )
            .await
        };
        tokio::select! {
            biased;
            _ = stop.changed() => Err("Download cancelled".into()),
            result = work => result,
        }
    }
    async fn download_with<F, FF, S>(
        &self,
        directory: &Path,
        raw: &str,
        name: &str,
        cache_key: Option<&str>,
        fetch: F,
    ) -> Result<String, String>
    where
        F: FnOnce() -> FF,
        FF: std::future::Future<Output = Result<(Option<u64>, S), String>>,
        S: Stream<Item = Result<bytes::Bytes, String>> + Send,
    {
        let destination = destination(directory, raw, name, cache_key)?;
        let key = destination.to_string_lossy().into_owned();
        self.locks
            .with_lock(key, async {
                if cache_key.is_some()
                    && tokio::fs::metadata(&destination)
                        .await
                        .is_ok_and(|metadata| metadata.is_file())
                {
                    return Ok(destination.to_string_lossy().into_owned());
                }
                let _permit = self
                    .permits
                    .acquire()
                    .await
                    .map_err(|error| error.to_string())?;
                let (expected, stream) = fetch().await?;
                let temporary = write_download(directory, expected, stream).await?;
                temporary
                    .persist_noclobber(&destination)
                    .map_err(|error| error.to_string())?;
                Ok(destination.to_string_lossy().into_owned())
            })
            .await
    }
}
async fn write_download<S>(
    directory: &Path,
    expected: Option<u64>,
    stream: S,
) -> Result<tempfile::NamedTempFile, String>
where
    S: Stream<Item = Result<bytes::Bytes, String>> + Send,
{
    let parent = directory.to_path_buf();
    let temporary = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&parent)?;
        tempfile::Builder::new()
            .prefix("remote-")
            .tempfile_in(parent)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    let mut output =
        tokio::fs::File::from_std(temporary.reopen().map_err(|error| error.to_string())?);
    let mut size = 0u64;
    tokio::pin!(stream);
    loop {
        let chunk = tokio::time::timeout(Duration::from_secs(60), stream.next())
            .await
            .map_err(|_| "Download body timed out")?;
        let Some(chunk) = chunk else { break };
        let chunk = chunk?;
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or("Download too large")?;
        if expected.is_some_and(|expected| size > expected) {
            return Err("Download exceeds declared length".into());
        }
        output
            .write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
    }
    if expected.is_some_and(|expected| size != expected) {
        return Err("Download ended before declared length".into());
    }
    output.flush().await.map_err(|error| error.to_string())?;
    drop(output.into_std().await);
    Ok(temporary)
}

fn url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("HTTP download URL required".into());
    }
    Ok(url)
}
fn destination(
    directory: &Path,
    raw: &str,
    name: &str,
    cache_key: Option<&str>,
) -> Result<PathBuf, String> {
    let url = url(raw)?;
    let name = if name.is_empty() {
        url.path().rsplit('/').next().unwrap_or_default()
    } else {
        name
    };
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .filter(|extension| {
            !extension.is_empty()
                && extension.len() <= 16
                && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        });
    let key = if let Some(cache_key) = cache_key {
        let mut hash = Sha256::new();
        let origin = url.origin().ascii_serialization();
        for part in ["remote-file-v1", origin.as_str(), name, cache_key] {
            hash.update((part.len() as u64).to_be_bytes());
            hash.update(part.as_bytes());
        }
        plain_rs::utils::hex::bytes_to_hex(&hash.finalize())
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    Ok(directory.join(match extension {
        Some(extension) => format!("{key}.{extension}"),
        None => key,
    }))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/remote_cache.rs"]
mod tests;
