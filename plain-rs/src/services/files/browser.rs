use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Operation {
    Remux,
    Transcode,
}
impl Operation {
    fn capability(self) -> &'static str {
        match self {
            Self::Remux => "VIDEO_REMUX",
            Self::Transcode => "VIDEO_TRANSCODE",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Remux => "remux",
            Self::Transcode => "h264",
        }
    }
}
#[derive(Eq, PartialEq)]
struct Source {
    size: u64,
    modified: SystemTime,
}
impl Source {
    async fn read(path: &Path) -> Result<Self, String> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            return Err("Regular video file required".into());
        }
        Ok(Self {
            size: metadata.len(),
            modified: metadata.modified().map_err(|error| error.to_string())?,
        })
    }
}
struct Output(PathBuf);
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub struct Runtime {
    cache: PathBuf,
    conversion: tokio::sync::Mutex<()>,
}
impl Runtime {
    pub fn new(cache: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            cache,
            conversion: tokio::sync::Mutex::new(()),
        })
    }
    pub async fn convert(
        &self,
        path: &str,
        operation: Operation,
    ) -> Result<Option<PathBuf>, String> {
        self.convert_with(
            path,
            operation,
            || async { Ok(capabilities().into_iter().map(str::to_owned).collect()) },
            |input, output| async move {
                #[cfg(target_os = "android")]
                {
                    return encode_android(
                        input,
                        output,
                        match operation {
                            Operation::Remux => {
                                plain_rs::platform::android::media::Operation::Remux
                            }
                            Operation::Transcode => {
                                plain_rs::platform::android::media::Operation::Transcode
                            }
                        },
                    )
                    .await;
                }
                #[cfg(all(feature = "runtime-media", not(target_os = "android")))]
                {
                    plain_rs::services::files::videos_preview::encode_browser_video(
                        &input,
                        &output,
                        operation == Operation::Transcode,
                    )
                    .await
                    .map_err(|error| error.to_string())
                }
                #[cfg(all(not(feature = "runtime-media"), not(target_os = "android")))]
                {
                    let _ = (input, output);
                    Err("Video encoder unavailable".into())
                }
            },
        )
        .await
    }
    async fn convert_with<C, CF, E, EF>(
        &self,
        path: &str,
        operation: Operation,
        capabilities: C,
        encode: E,
    ) -> Result<Option<PathBuf>, String>
    where
        C: FnOnce() -> CF,
        CF: std::future::Future<Output = Result<Vec<String>, String>>,
        E: FnOnce(PathBuf, PathBuf) -> EF,
        EF: std::future::Future<Output = Result<bool, String>>,
    {
        let _conversion = self.conversion.lock().await;
        let source = PathBuf::from(path);
        let before = Source::read(&source).await?;
        if before.size == 0 {
            return Ok(None);
        }
        let input = source.clone();
        let decision = tokio::task::spawn_blocking(move || {
            let Some(tracks) = plain_rs::media::isobmff::tracks(&input) else {
                return false;
            };
            match operation {
                Operation::Remux => tracks.needs_remux,
                Operation::Transcode => {
                    matches!(tracks.first_video_codec.as_deref(), Some("hvc1" | "hev1"))
                        && plain_rs::media::isobmff::mp4_milliseconds(&input).unwrap_or(0)
                            <= 600_000
                }
            }
        })
        .await
        .map_err(|error| error.to_string())?;
        if !decision {
            return Ok(None);
        }
        let capabilities = capabilities().await?;
        if !capabilities
            .iter()
            .any(|name| name == operation.capability())
        {
            return Ok(None);
        }
        let destination = cache_path(&self.cache, path, &before, operation);
        if tokio::fs::metadata(&destination)
            .await
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        {
            return Ok(Some(destination));
        }
        let parent = destination.parent().ok_or("Missing video cache parent")?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
        let temporary = Output(parent.join(format!("{}.mp4", uuid::Uuid::new_v4())));
        tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary.0)
            .await
            .map_err(|error| error.to_string())?;
        if !encode(source.clone(), temporary.0.clone()).await? {
            return Ok(None);
        }
        let metadata = tokio::fs::metadata(&temporary.0)
            .await
            .map_err(|error| error.to_string())?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err("Video encoder produced no output".into());
        }
        if Source::read(&source).await? != before {
            return Err("Source changed during video conversion".into());
        }
        tokio::fs::rename(&temporary.0, &destination)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Some(destination))
    }
}
fn cache_path(root: &Path, path: &str, source: &Source, operation: Operation) -> PathBuf {
    let mut hash = Sha256::new();
    hash.update(b"browser-video-v1\0");
    hash.update(operation.key());
    hash.update([0]);
    hash.update(path);
    hash.update(source.size.to_be_bytes());
    let modified = chrono::DateTime::<chrono::Utc>::from(source.modified);
    hash.update(modified.timestamp().to_be_bytes());
    hash.update(modified.timestamp_subsec_nanos().to_be_bytes());
    let key = plain_rs::utils::hex::bytes_to_hex(&hash.finalize());
    root.join("videos")
        .join(&key[..2])
        .join(format!("{key}.mp4"))
}
#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
fn declared_capabilities(available: bool) -> Vec<&'static str> {
    if available {
        vec!["VIDEO_REMUX", "VIDEO_TRANSCODE", "VIDEO_3GP_CONVERT"]
    } else {
        Vec::new()
    }
}
pub fn capabilities() -> Vec<&'static str> {
    #[cfg(target_os = "android")]
    {
        plain_rs::platform::android::media::capabilities()
    }
    #[cfg(target_os = "ios")]
    {
        Vec::new()
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let ffmpeg = cfg!(feature = "runtime-media")
            && std::env::var_os("PATH").is_some_and(|paths| {
                std::env::split_paths(&paths).any(|path| {
                    let file = path.join(if cfg!(windows) {
                        "ffmpeg.exe"
                    } else {
                        "ffmpeg"
                    });
                    let Ok(metadata) = file.metadata() else {
                        return false;
                    };
                    if !metadata.is_file() {
                        return false;
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        metadata.permissions().mode() & 0o111 != 0
                    }
                    #[cfg(not(unix))]
                    {
                        true
                    }
                })
            });
        declared_capabilities(ffmpeg)
    }
}

#[cfg(target_os = "android")]
pub async fn encode_android(
    input: PathBuf,
    output: PathBuf,
    operation: plain_rs::platform::android::media::Operation,
) -> Result<bool, String> {
    android_task(move |cancel| {
        plain_rs::platform::android::media::encode(&input, &output, operation, cancel)
    })
    .await
}

#[cfg(target_os = "android")]
pub async fn encode_android_file(
    input: plain_rs::host::resource_stream::NativeResourceFile,
    output: PathBuf,
    operation: plain_rs::platform::android::media::Operation,
    stop: tokio::sync::watch::Receiver<bool>,
) -> Result<bool, String> {
    let directory = output.parent().ok_or("Missing media output directory")?;
    let input = plain_rs::services::files::input::prepare(input, directory, stop.clone()).await?;
    let encoding = android_task(move |cancel| {
        plain_rs::platform::android::media::encode_file(
            input.file,
            input.offset,
            input.length,
            &output,
            operation,
            cancel,
        )
    });
    let mut stop = stop;
    tokio::select! {
        biased;
        _ = stop.changed() => Err("Video conversion cancelled".into()),
        result = encoding => result,
    }
}

#[cfg(target_os = "android")]
async fn android_task<F>(encode: F) -> Result<bool, String>
where
    F: FnOnce(&std::sync::atomic::AtomicBool) -> anyhow::Result<bool> + Send + 'static,
{
    struct Cancellation(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Cancellation {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Release);
        }
    }
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _cancellation = Cancellation(cancelled.clone());
    tokio::task::spawn_blocking(move || encode(&cancelled))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/browser.rs"]
mod tests;
