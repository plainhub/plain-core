use anyhow::{Result, anyhow, ensure};
use futures_util::StreamExt;
use plain_rs::prefs::pref_keys::SKIP_VERSION;
use plain_rs::prefs::security::UpdateInfo;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::settings::updates;
use plain_rs::types::Long;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, sync::watch};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, async_graphql::Enum,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppUpdateDownloadStatus {
    #[default]
    Idle,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, async_graphql::SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateDownload {
    pub core_id: async_graphql::ID,
    pub task_id: async_graphql::ID,
    pub revision: Long,
    pub status: AppUpdateDownloadStatus,
    pub progress: i32,
    pub file_path: String,
    pub error: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Identity {
    version: String,
    url: String,
    size: u64,
}
impl From<&UpdateInfo> for Identity {
    fn from(info: &UpdateInfo) -> Self {
        Self {
            version: info.new_version.clone(),
            url: info.download_url.clone(),
            size: info.size,
        }
    }
}
struct Task {
    cancel: watch::Sender<bool>,
    join: tokio::task::JoinHandle<()>,
}
struct State {
    snapshot: AppUpdateDownload,
    cancel: Option<watch::Sender<bool>>,
    recovery_pending: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: AppUpdateDownload {
                core_id: plain_rs::utils::short_uuid::short_uuid().into(),
                ..Default::default()
            },
            cancel: None,
            recovery_pending: false,
        }
    }
}
#[derive(Default)]
pub struct Runtime {
    command: tokio::sync::Mutex<Option<Task>>,
    state: Mutex<State>,
    closed: AtomicBool,
}
impl Runtime {
    pub fn schedule_reconcile(state: &CoreRuntime) {
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = reconcile(&state).await {
                if !*state.stop.borrow() {
                    log::debug!("Update download recovery failed: {error}");
                }
            }
        });
    }
    pub fn notify_snapshot(&self, host: &plain_rs::host::bridge::Host) -> Result<(), String> {
        let current = self.state.lock().unwrap();
        let snapshot =
            serde_json::to_value(&current.snapshot).map_err(|error| error.to_string())?;
        host.notify("appUpdateDownloadChanged", snapshot)
    }
    fn publish(&self, state: &CoreRuntime, mut snapshot: AppUpdateDownload) -> AppUpdateDownload {
        let mut current = self.state.lock().unwrap();
        snapshot.core_id = current.snapshot.core_id.clone();
        snapshot.revision = Long(current.snapshot.revision.0 + 1);
        current.snapshot = snapshot.clone();
        if let Err(error) = state.host.notify(
            "appUpdateDownloadChanged",
            serde_json::to_value(&snapshot).unwrap(),
        ) {
            log::warn!("Update download callback: {error}");
            if !current.recovery_pending && state.host.connected() {
                current.recovery_pending = true;
                let state = state.clone();
                tokio::spawn(async move {
                    let permit = state.host.reserve_callback().await;
                    let mut current = state.update_checks.download.state.lock().unwrap();
                    if let Ok(permit) = permit {
                        if let Err(error) = state.host.notify_reserved_callback(
                            "appUpdateDownloadChanged",
                            serde_json::to_value(&current.snapshot).unwrap(),
                            permit,
                        ) {
                            log::warn!("Update download recovery callback: {error}");
                        }
                    }
                    current.recovery_pending = false;
                });
            }
        }
        snapshot
    }
    pub(super) fn invalidate(&self) {
        if let Some(cancel) = &self.state.lock().unwrap().cancel {
            cancel.send_replace(true);
        }
    }
    pub async fn lock_for_cache_clear(&self) -> impl Send + '_ {
        let mut task = self.command.lock().await;
        stop(&mut task).await;
        task
    }
    pub fn reset_after_cache_clear(&self, state: &CoreRuntime) -> Result<()> {
        updates::update(state, serde_json::json!({"downloadedApkPath":""}))?;
        self.state.lock().unwrap().cancel = None;
        self.publish(state, AppUpdateDownload::default());
        Ok(())
    }
    pub async fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        let mut task = self.command.lock().await;
        stop(&mut task).await;
    }
}
async fn stop(task: &mut Option<Task>) {
    if let Some(task) = task.take() {
        task.cancel.send_replace(true);
        if let Err(error) = task.join.await {
            log::error!("Update download task: {error}");
        }
    }
}

#[derive(Clone, Copy)]
pub enum Command {
    Reconcile,
    Start,
    Cancel,
}

pub async fn reconcile(state: &CoreRuntime) -> Result<()> {
    ensure!(!*state.stop.borrow(), "Rust core stopped");
    let capabilities = plain_rs::services::capabilities::snapshot(&state.host).await?;
    if capabilities
        .iter()
        .any(|capability| capability == "APP_UPDATES")
    {
        execute_owned(state.clone(), Command::Reconcile).await?;
    }
    Ok(())
}

pub async fn execute(state: &CoreRuntime, command: Command) -> Result<AppUpdateDownload> {
    plain_rs::services::capabilities::require(&state.host, "APP_UPDATES").await?;
    let state = state.clone();
    tokio::spawn(async move { execute_owned(state, command).await }).await?
}
async fn execute_owned(state: CoreRuntime, command: Command) -> Result<AppUpdateDownload> {
    let runtime = state.update_checks.download.clone();
    let mut task = runtime.command.lock().await;
    ensure!(
        !runtime.closed.load(Ordering::Acquire) && !*state.stop.borrow(),
        "Rust core stopped"
    );
    if !matches!(command, Command::Reconcile) {
        stop(&mut task).await;
    }
    let _settings = state.update_checks.cache.lock().await;
    ensure!(!runtime.closed.load(Ordering::Acquire), "Rust core stopped");
    let info = updates::info(&state)?;
    match command {
        Command::Start => {
            let version = &state.host.startup().app_version;
            ensure!(
                plain_rs::updates::available(
                    &info,
                    version,
                    &state.prefs.get_or(SKIP_VERSION, String::new())
                ),
                "No application update available"
            );
            let identity = Identity::from(&info);
            validate_url(&identity.url)?;
            discard(&state, &info.downloaded_apk_path)?;
            updates::update(&state, serde_json::json!({"downloadedApkPath":""}))?;
            let id = plain_rs::utils::short_uuid::short_uuid();
            let (cancel, receiver) = watch::channel(false);
            runtime.state.lock().unwrap().cancel = Some(cancel.clone());
            let snapshot = runtime.publish(
                &state,
                AppUpdateDownload {
                    task_id: id.clone().into(),
                    status: AppUpdateDownloadStatus::Downloading,
                    ..Default::default()
                },
            );
            let worker_state = state.clone();
            let join = tokio::spawn(async move { run(worker_state, id, identity, receiver).await });
            *task = Some(Task { cancel, join });
            Ok(snapshot)
        }
        Command::Cancel => {
            discard(&state, &info.downloaded_apk_path)?;
            updates::update(&state, serde_json::json!({"downloadedApkPath":""}))?;
            runtime.state.lock().unwrap().cancel = None;
            Ok(runtime.publish(&state, AppUpdateDownload::default()))
        }
        Command::Reconcile => {
            let snapshot = runtime.state.lock().unwrap().snapshot.clone();
            if snapshot.status == AppUpdateDownloadStatus::Downloading {
                return Ok(snapshot);
            }
            let restored = match restored(&state, &info) {
                Ok(restored) => restored,
                Err(error) => {
                    if !*state.stop.borrow() {
                        runtime.publish(
                            &state,
                            AppUpdateDownload {
                                status: AppUpdateDownloadStatus::Failed,
                                error: error.to_string(),
                                ..Default::default()
                            },
                        );
                    }
                    return Err(error);
                }
            };
            if let Some(path) = restored {
                if snapshot.status == AppUpdateDownloadStatus::Completed
                    && snapshot.file_path == path
                {
                    return Ok(snapshot);
                }
                return Ok(runtime.publish(
                    &state,
                    AppUpdateDownload {
                        status: AppUpdateDownloadStatus::Completed,
                        progress: 100,
                        file_path: path,
                        ..Default::default()
                    },
                ));
            }
            if !info.downloaded_apk_path.is_empty() {
                updates::update(&state, serde_json::json!({"downloadedApkPath":""}))?;
            }
            if snapshot.status == AppUpdateDownloadStatus::Completed {
                return Ok(runtime.publish(&state, AppUpdateDownload::default()));
            }
            Ok(snapshot)
        }
    }
}
fn validate_url(raw: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(raw)?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "Invalid update download URL"
    );
    Ok(url)
}
fn client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .use_rustls_tls()
                .https_only(true)
                .user_agent("PlainHub")
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(1800))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow!(error.clone()))
}
async fn cancelled(cancel: &mut watch::Receiver<bool>, stop: &mut watch::Receiver<bool>) {
    if *cancel.borrow() || *stop.borrow() {
        return;
    }
    tokio::select! { _=cancel.changed()=>{}, _=stop.changed()=>{} }
}
async fn run(
    state: CoreRuntime,
    id: String,
    identity: Identity,
    mut cancel: watch::Receiver<bool>,
) {
    let result = transfer(&state, &id, &identity, &mut cancel).await;
    let runtime = &state.update_checks.download;
    let snapshot = match result {
        Ok(path) => AppUpdateDownload {
            task_id: id.into(),
            status: AppUpdateDownloadStatus::Completed,
            progress: 100,
            file_path: path,
            ..Default::default()
        },
        Err(error) => AppUpdateDownload {
            task_id: id.into(),
            status: if *cancel.borrow() || *state.stop.borrow() {
                AppUpdateDownloadStatus::Cancelled
            } else {
                AppUpdateDownloadStatus::Failed
            },
            error: error.to_string(),
            ..Default::default()
        },
    };
    runtime.publish(&state, snapshot);
}
async fn transfer(
    state: &CoreRuntime,
    id: &str,
    identity: &Identity,
    cancel: &mut watch::Receiver<bool>,
) -> Result<String> {
    let mut stop = state.stop.clone();
    let response = tokio::select! {
        biased;
        _=cancelled(cancel, &mut stop)=>return Err(anyhow!("Update download cancelled")),
        response=client()?.get(validate_url(&identity.url)?).send()=>response?,
    }
    .error_for_status()?;
    let total = response.content_length();
    let stream = response
        .bytes_stream()
        .map(|chunk| chunk.map_err(anyhow::Error::from));
    save_stream(state, id, identity, stream, total, cancel).await
}
async fn save_stream<B: AsRef<[u8]>>(
    state: &CoreRuntime,
    id: &str,
    identity: &Identity,
    stream: impl futures_util::Stream<Item = Result<B>>,
    total: Option<u64>,
    cancel: &mut watch::Receiver<bool>,
) -> Result<String> {
    if let Some(total) = total {
        ensure!(
            identity.size == 0 || total == identity.size,
            "Update download size mismatch"
        );
    }
    let root = state.cache.app_updates();
    std::fs::create_dir_all(&root)?;
    ensure!(
        std::fs::symlink_metadata(&root)?.file_type().is_dir(),
        "Invalid update cache directory"
    );
    let staging = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(&root)?;
    let mut stop = state.stop.clone();
    futures_util::pin_mut!(stream);
    let mut output = tokio::fs::File::create(staging.path().join("package")).await?;
    let mut downloaded = 0u64;
    let mut last_progress = 0;
    let result: Result<()> = async {
        loop {
            let chunk = tokio::select! {
                biased;
                _=cancelled(cancel, &mut stop)=>return Err(anyhow!("Update download cancelled")),
                chunk=stream.next()=>chunk,
            };
            let Some(chunk) = chunk else {
                break;
            };
            let chunk = chunk?;
            let chunk = chunk.as_ref();
            downloaded = downloaded
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| anyhow!("Update download too large"))?;
            ensure!(
                identity.size == 0 || downloaded <= identity.size,
                "Update download size mismatch"
            );
            output.write_all(chunk).await?;
            let progress = percentage(
                downloaded,
                total.or((identity.size > 0).then_some(identity.size)),
            );
            if progress != last_progress {
                state.update_checks.download.publish(
                    state,
                    AppUpdateDownload {
                        task_id: id.into(),
                        status: AppUpdateDownloadStatus::Downloading,
                        progress,
                        ..Default::default()
                    },
                );
                last_progress = progress;
            }
        }
        ensure!(
            downloaded > 0
                && (identity.size == 0 || downloaded == identity.size)
                && total.is_none_or(|total| total == downloaded),
            "Incomplete update download"
        );
        output.flush().await?;
        Ok(())
    }
    .await;
    drop(output);
    result?;
    let _settings = tokio::select! {
        biased;
        _=cancelled(cancel, &mut stop)=>return Err(anyhow!("Update download cancelled")),
        settings=state.update_checks.cache.lock()=>settings,
    };
    ensure!(
        !*cancel.borrow() && !*stop.borrow(),
        "Update download cancelled"
    );
    tokio::select! {
        biased;
        _=cancelled(cancel, &mut stop)=>return Err(anyhow!("Update download cancelled")),
        result=plain_rs::services::capabilities::require(&state.host, "APP_UPDATES")=>result?,
    }
    ensure!(
        !*cancel.borrow() && !*stop.borrow(),
        "Update download cancelled"
    );
    let current = updates::info(&state)?;
    let version = &state.host.startup().app_version;
    ensure!(
        Identity::from(&current) == *identity
            && plain_rs::updates::available(
                &current,
                version,
                &state.prefs.get_or(SKIP_VERSION, String::new())
            ),
        "Update source changed during download"
    );
    std::fs::write(
        staging.path().join("source.json"),
        serde_json::to_vec(identity)?,
    )?;
    let target = root.join(id);
    std::fs::rename(staging.path(), &target)?;
    let path = target.join("package").to_string_lossy().into_owned();
    if let Err(error) = updates::update(state, serde_json::json!({"downloadedApkPath":path})) {
        std::fs::remove_dir_all(&target)?;
        return Err(error);
    }
    Ok(path)
}
fn percentage(downloaded: u64, total: Option<u64>) -> i32 {
    total
        .filter(|total| *total > 0)
        .map(|total| ((downloaded as u128 * 100 / total as u128).min(99)) as i32)
        .unwrap_or(0)
}
fn restored(state: &CoreRuntime, info: &UpdateInfo) -> Result<Option<String>> {
    let version = &state.host.startup().app_version;
    if info.downloaded_apk_path.is_empty()
        || !plain_rs::updates::available(
            info,
            version,
            &state.prefs.get_or(SKIP_VERSION, String::new()),
        )
    {
        return Ok(None);
    }
    let path = Path::new(&info.downloaded_apk_path);
    let Some(parent) = owned_directory(state, path) else {
        return Ok(None);
    };
    if !std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file()) {
        return Ok(None);
    }
    let metadata = parent.join("source.json");
    if !std::fs::symlink_metadata(&metadata)
        .is_ok_and(|metadata| metadata.file_type().is_file() && metadata.len() <= 4096)
    {
        return Ok(None);
    }
    let Ok(saved) = serde_json::from_slice::<Identity>(&std::fs::read(metadata)?) else {
        return Ok(None);
    };
    let length = std::fs::metadata(path)?.len();
    Ok(
        (saved == Identity::from(info) && length > 0 && (saved.size == 0 || saved.size == length))
            .then(|| info.downloaded_apk_path.clone()),
    )
}

fn owned_directory(state: &CoreRuntime, path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    let id = parent.file_name()?.to_str()?;
    if path.file_name()? != "package"
        || parent.parent()? != state.cache.app_updates()
        || id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !std::fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.file_type().is_dir())
    {
        return None;
    }
    Some(parent.to_owned())
}
fn discard(state: &CoreRuntime, path: &str) -> Result<()> {
    if let Some(directory) = owned_directory(state, Path::new(path)) {
        std::fs::remove_dir_all(directory)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/settings/update_download.rs"]
mod tests;

#[cfg(any(test, feature = "test-seams"))]
pub mod test_fixture {
    use super::*;
    pub async fn save(state: &CoreRuntime, id: &str, chunks: Vec<Vec<u8>>) -> Result<String> {
        let identity = Identity::from(&updates::info(state)?);
        let total = chunks.iter().map(|chunk| chunk.len() as u64).sum();
        let (_cancel, mut receiver) = watch::channel(false);
        let stream = futures_util::stream::iter(chunks.into_iter().map(Ok::<_, anyhow::Error>));
        save_stream(state, id, &identity, stream, Some(total), &mut receiver).await
    }
}
