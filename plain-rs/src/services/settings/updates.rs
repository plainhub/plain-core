use anyhow::{Result, anyhow, ensure};
use plain_rs::prefs::pref_keys::{AUTO_CHECK_UPDATE, SKIP_VERSION};
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::{events::WsEvent, prefs::security::UpdateInfo, updates::Release};
use std::{sync::OnceLock, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppUpdateStatus {
    Available,
    UpToDate,
    RateLimited,
    NotDue,
}

fn client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .use_rustls_tls()
                .user_agent("PlainHub")
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow!(error.clone()))
}

pub fn info(state: &CoreRuntime) -> Result<UpdateInfo> {
    Ok(state
        .store
        .get()?
        .get_json("update_info")?
        .unwrap_or_default())
}

pub fn update(state: &CoreRuntime, patch: serde_json::Value) -> Result<()> {
    let patch: plain_rs::prefs::security::UpdatePatch = serde_json::from_value(patch)?;
    state
        .store
        .get()?
        .update_json("update_info", |info: &mut UpdateInfo| {
            patch.apply(info);
            Ok(())
        })?;
    state.prefs.notify_changed();
    let _ = state
        .events
        .send(WsEvent::broadcast("PREFS_UPDATED", "{}".into()));
    Ok(())
}

#[derive(Default)]
pub(super) struct Cache {
    checked_at: Option<tokio::time::Instant>,
    outcome: Option<Result<AppUpdateStatus, String>>,
}
#[derive(Default)]
pub struct Runtime {
    pub(super) cache: tokio::sync::Mutex<Cache>,
    pub download: std::sync::Arc<plain_rs::services::settings::update_download::Runtime>,
}

pub async fn check(state: &CoreRuntime, automatic: bool) -> Result<AppUpdateStatus> {
    ensure!(!*state.stop.borrow(), "Rust core stopped");
    plain_rs::services::capabilities::require(&state.host, "APP_UPDATES").await?;
    let mut cache = state.update_checks.cache.lock().await;
    let now_ms = u64::try_from(chrono::Utc::now().timestamp_millis())?;
    let info = info(state)?;
    if automatic
        && !plain_rs::updates::due(&info, state.prefs.get_or(AUTO_CHECK_UPDATE, true), now_ms)
    {
        return Ok(AppUpdateStatus::NotDue);
    }
    attempt(state, &mut cache).await
}

pub async fn snapshot(
    state: &CoreRuntime,
) -> Result<plain_rs::services::settings::types::AppUpdate> {
    ensure!(!*state.stop.borrow(), "Rust core stopped");
    plain_rs::services::capabilities::require(&state.host, "APP_UPDATES").await?;
    let mut cache = state.update_checks.cache.lock().await;
    let status = if cache
        .checked_at
        .is_some_and(|at| at.elapsed() < Duration::from_secs(600))
    {
        cache
            .outcome
            .clone()
            .ok_or_else(|| anyhow!("Update check unavailable"))?
            .map_err(anyhow::Error::msg)?
    } else {
        attempt(state, &mut cache).await?
    };
    ensure!(
        status != AppUpdateStatus::RateLimited,
        "Update source rate limited"
    );
    project(state)
}

fn project(state: &CoreRuntime) -> Result<plain_rs::services::settings::types::AppUpdate> {
    let info = info(state)?;
    let version = &state.host.startup().app_version;
    let latest = info.latest_release.as_ref();
    Ok(plain_rs::services::settings::types::AppUpdate {
        current_version: plain_rs::updates::normalized_version(version),
        latest_version: latest.map(|release| release.version.clone()),
        has_update: latest.is_some_and(|release| {
            plain_rs::updates::available_version(
                &release.version,
                version,
                &state.prefs.get_or(SKIP_VERSION, String::new()),
            )
        }),
        url: latest
            .map(|release| release.url.clone())
            .filter(|url| !url.is_empty()),
        release_name: latest
            .map(|release| release.name.clone())
            .unwrap_or_default(),
        published_at: latest
            .and_then(|release| chrono::DateTime::parse_from_rfc3339(&release.published_at).ok())
            .map(|time| plain_rs::types::Instant(time.with_timezone(&chrono::Utc))),
    })
}

async fn attempt(state: &CoreRuntime, cache: &mut Cache) -> Result<AppUpdateStatus> {
    let mut stop = state.stop.clone();
    ensure!(!*stop.borrow(), "Rust core stopped");
    let result = tokio::select! {
        biased;
        _=stop.changed()=>Err(anyhow!("Rust core stopped")),
        result=fetch(state)=>result,
    };
    cache.checked_at = Some(tokio::time::Instant::now());
    cache.outcome = Some(result.as_ref().copied().map_err(ToString::to_string));
    result
}

async fn fetch(state: &CoreRuntime) -> Result<AppUpdateStatus> {
    let now_ms = u64::try_from(chrono::Utc::now().timestamp_millis())?;
    let startup = state.host.startup();
    let source = startup
        .update_source
        .as_ref()
        .ok_or_else(|| anyhow!("Missing update source"))?;
    let abis = &startup.supported_abis;
    let version = &startup.app_version;
    let url = reqwest::Url::parse(&source.url)?;
    ensure!(
        url.scheme() == "https" && url.host_str().is_some(),
        "Invalid update source"
    );
    let response = client()?.get(url).send().await?;
    update(state, serde_json::json!({"checkUpdateTime":now_ms}))?;
    if response.status() == reqwest::StatusCode::FORBIDDEN
        || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return Ok(AppUpdateStatus::RateLimited);
    }
    let response = response.error_for_status()?;
    ensure!(
        response.content_length().unwrap_or(0) <= 4 * 1024 * 1024,
        "Update response too large"
    );
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        ensure!(
            bytes.len() + chunk.len() <= 4 * 1024 * 1024,
            "Update response too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    let release: Release = serde_json::from_slice(&bytes)?;
    let latest = plain_rs::updates::release_info(&release, source)?;
    let mut info: UpdateInfo = info(state)?;
    let available = plain_rs::updates::apply_release(
        &mut info,
        version,
        &state.prefs.get_or(SKIP_VERSION, String::new()),
        abis,
        source,
        &release,
    );
    let mut patch = serde_json::json!({"latestRelease":latest});
    if available {
        patch.as_object_mut().unwrap().extend(serde_json::json!({"newVersion":info.new_version,"log":info.log,"publishDate":info.publish_date,"size":info.size,"downloadUrl":info.download_url}).as_object().unwrap().clone());
    }
    let previous = plain_rs::services::settings::updates::info(state)?;
    if previous.new_version != info.new_version
        || previous.download_url != info.download_url
        || previous.size != info.size
    {
        state.update_checks.download.invalidate();
        patch["downloadedApkPath"] = serde_json::json!("");
    }
    update(state, patch)?;
    plain_rs::services::settings::update_download::Runtime::schedule_reconcile(state);
    if !available {
        return Ok(AppUpdateStatus::UpToDate);
    }

    Ok(AppUpdateStatus::Available)
}

pub async fn set_setting(state: &CoreRuntime, key: &str, value: serde_json::Value) -> Result<bool> {
    let _cache = state.update_checks.cache.lock().await;
    let changed = plain_rs::prefs::security::set_entry(&state.prefs, key, value)?;
    if changed && key == SKIP_VERSION {
        state.update_checks.download.invalidate();
        update(state, serde_json::json!({"downloadedApkPath":""}))?;
        plain_rs::services::settings::update_download::Runtime::schedule_reconcile(state);
    }
    Ok(changed)
}

#[cfg(any(test, feature = "test-seams"))]
impl Runtime {
    pub async fn seed_cache_for_test(&self, status: AppUpdateStatus) {
        let mut cache = self.cache.lock().await;
        cache.checked_at = Some(tokio::time::Instant::now());
        cache.outcome = Some(Ok(status));
    }
}
