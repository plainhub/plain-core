use plain_rs::host::bridge::Host;
use plain_rs::prefs::pref_keys::AI_IMAGE_SEARCH_MODEL;
use plain_rs::services::images::index::ImageIndex;
use plain_rs::services::images::status::ImageSearchStatus;
use plain_rs::services::images::status::ImageSearchStatusType;

use plain_rs::events::WsEvent;
use plain_rs::prefs::Prefs;
use serde::Serialize;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: u64,
    #[serde(flatten)]
    pub status: ImageSearchStatus,
}
impl Snapshot {
    pub fn event(&self) -> WsEvent {
        WsEvent::broadcast("IMAGE_MODELS_UPDATED", serde_json::to_string(self).unwrap())
    }
}
struct ModelState {
    version: u64,
    published_version: Option<u64>,
    fingerprint: String,
    status: ImageSearchStatusType,
    progress: i32,
    error: String,
}
#[derive(Clone, Copy)]
enum Selection {
    Default,
    Imported,
    Active,
    Incoming,
}

pub struct Runtime {
    directory: PathBuf,
    host: Arc<Host>,
    prefs: Arc<Prefs>,
    store: Arc<plain_rs::kv::LazyDb>,
    index: Arc<ImageIndex>,
    state: Mutex<ModelState>,
    #[cfg(any(test, feature = "test-seams"))]
    snapshot_reads: std::sync::atomic::AtomicUsize,
    engine: Mutex<Option<Arc<plain_inference::embedding::Engine>>>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    operations: tokio::sync::Mutex<()>,
    cancelled: AtomicBool,
    observing: AtomicBool,
    catalog_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    events: tokio::sync::broadcast::Sender<WsEvent>,
}
impl Runtime {
    pub fn new(
        directory: PathBuf,
        host: Arc<Host>,
        prefs: Arc<Prefs>,
        store: Arc<plain_rs::kv::LazyDb>,
        index: Arc<ImageIndex>,
        events: tokio::sync::broadcast::Sender<WsEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            directory: directory.join("ai_models"),
            host,
            prefs,
            store,
            index,
            events,
            state: Mutex::new(ModelState {
                version: 0,
                published_version: None,
                fingerprint: String::new(),
                status: ImageSearchStatusType::Unavailable,
                progress: 0,
                error: String::new(),
            }),
            #[cfg(any(test, feature = "test-seams"))]
            snapshot_reads: std::sync::atomic::AtomicUsize::new(0),
            engine: Mutex::new(None),
            task: Mutex::new(None),
            operations: tokio::sync::Mutex::new(()),
            cancelled: AtomicBool::new(false),
            observing: AtomicBool::new(false),
            catalog_task: Mutex::new(None),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        #[cfg(any(test, feature = "test-seams"))]
        self.snapshot_reads.fetch_add(1, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        let index = self.index.status();
        let mut snapshot = Snapshot {
            version: 0,
            status: ImageSearchStatus {
                status: state.status,
                download_progress: state.progress,
                error_message: if index.error_message.is_empty() {
                    state.error.clone()
                } else {
                    index.error_message
                },
                model_size: plain_rs::types::Long(
                    self.engine.lock().unwrap().as_ref().map_or_else(
                        || plain_inference::models::default_model::manifest().size(),
                        |e| e.manifest.size(),
                    ) as i64,
                ),
                model_dir: self
                    .directory
                    .join("incoming")
                    .to_string_lossy()
                    .into_owned(),
                is_indexing: index.is_running,
                total_images: index.total_images.min(i32::MAX as usize) as i32,
                indexed_images: index.indexed_images.min(i32::MAX as usize) as i32,
            },
        };
        let fingerprint = serde_json::to_string(&snapshot).unwrap();
        if state.fingerprint != fingerprint {
            state.version += 1;
            state.fingerprint = fingerprint;
        }
        snapshot.version = state.version;
        snapshot
    }
    pub fn catalog_changed(self: &Arc<Self>) {
        if !self.observing.load(Ordering::Acquire) {
            return;
        }
        let mut task = self.catalog_task.lock().unwrap();
        if let Some(previous) = task.take() {
            previous.abort();
        }
        let owner = Arc::downgrade(self);
        *task = Some(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let Some(owner) = owner.upgrade() else {
                return;
            };
            let _guard = owner.operations.lock().await;
            if owner.observing.load(Ordering::Acquire) {
                if let Err(error) = owner.index.start(false) {
                    owner.report_error(error.to_string());
                }
            }
        }));
    }
    fn cancel_catalog_changes(&self) {
        if let Some(task) = self.catalog_task.lock().unwrap().take() {
            task.abort();
        }
    }
    fn report_error(&self, message: String) {
        self.state.lock().unwrap().error = message;
        self.publish();
    }
    fn publish(&self) {
        let snapshot = self.snapshot();
        let mut state = self.state.lock().unwrap();
        if state
            .published_version
            .is_some_and(|version| version >= snapshot.version)
        {
            return;
        }
        state.published_version = Some(snapshot.version);
        let _ = self.events.send(snapshot.event());
    }
    fn set_status(&self, status: ImageSearchStatusType, error: String) {
        {
            let mut state = self.state.lock().unwrap();
            state.status = status;
            state.error = error;
        }
        self.publish();
    }
    fn selected_model(&self) -> Result<&'static str, String> {
        match self
            .prefs
            .get_or(AI_IMAGE_SEARCH_MODEL, String::from("default"))
            .as_str()
        {
            "default" => Ok("default"),
            "imported" => Ok("imported"),
            _ => Err("Image search model must be default or imported".into()),
        }
    }
    pub async fn enable(self: &Arc<Self>, restore: bool) -> Result<(), String> {
        let model = self.selected_model()?;
        if restore
            && (!self.prefs.get_or("ai_image_search_enabled", false)
                || !self.directory.join(model).join("manifest.json").is_file())
        {
            return Ok(());
        }
        self.activate(if restore {
            Selection::Active
        } else if model == "imported" {
            Selection::Imported
        } else {
            Selection::Default
        })
        .await
    }
    pub async fn import(self: &Arc<Self>) -> Result<(), String> {
        if !self.directory.join("incoming/manifest.json").is_file() {
            return Err("Upload a complete model package first".into());
        }
        self.activate(Selection::Incoming).await
    }
    async fn activate(self: &Arc<Self>, selection: Selection) -> Result<(), String> {
        let _guard = self.operations.lock().await;
        if self
            .task
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|task| !task.is_finished())
        {
            return Ok(());
        }
        self.cancelled.store(false, Ordering::Release);
        let runtime = self.clone();
        *self.task.lock().unwrap() = Some(tokio::spawn(async move {
            if let Err(error) = runtime.load(selection).await {
                let status = if runtime.engine.lock().unwrap().is_some() {
                    ImageSearchStatusType::Ready
                } else {
                    ImageSearchStatusType::Error
                };
                runtime.set_status(status, error);
            }
        }));
        Ok(())
    }
    async fn load(&self, selection: Selection) -> Result<(), String> {
        let model = match selection {
            Selection::Default => "default",
            Selection::Incoming | Selection::Imported => "imported",
            Selection::Active => self.selected_model()?,
        };
        let active = self.directory.join(model);
        let default_is_current =
            plain_inference::models::manifest::Manifest::read(&active.join("manifest.json"))
                .ok()
                .is_some_and(|manifest| {
                    manifest.fingerprint().ok()
                        == plain_inference::models::default_model::manifest()
                            .fingerprint()
                            .ok()
                });
        let source = match selection {
            Selection::Incoming => self.directory.join("incoming"),
            Selection::Default if !default_is_current => {
                self.state.lock().unwrap().progress = 0;
                self.set_status(ImageSearchStatusType::Downloading, String::new());
                self.download().await?;
                self.directory.join("download")
            }
            _ => active.clone(),
        };
        self.set_status(ImageSearchStatusType::Loading, String::new());
        let manifest =
            plain_inference::models::manifest::Manifest::read(&source.join("manifest.json"))?;
        let fingerprint = manifest.fingerprint()?;
        self.pause_index().await?;
        if let Some(engine) = self.engine.lock().unwrap().as_ref() {
            engine.release_all();
        }
        let directory = source.clone();
        let engine = tokio::task::spawn_blocking(move || {
            plain_inference::embedding::Engine::open(directory, manifest)
        })
        .await
        .map_err(|e| e.to_string())??;
        if self.cancelled.load(Ordering::Acquire) {
            return Err("Model activation cancelled".into());
        }
        let previous = self.directory.join("previous");
        if source != active {
            if previous.exists() {
                std::fs::remove_dir_all(&previous).map_err(|e| e.to_string())?;
            }
            if active.exists() {
                std::fs::rename(&active, &previous).map_err(|e| e.to_string())?;
            }
            if let Err(error) = std::fs::rename(&source, &active) {
                if previous.exists() {
                    let _ = std::fs::rename(&previous, &active);
                }
                return Err(error.to_string());
            }
        }
        let engine = Arc::new(engine.relocate(active));
        let store = self.store.get().map_err(|e| e.to_string())?;
        if store
            .get_json::<String>("image_index_model")
            .map_err(|e| e.to_string())?
            .as_deref()
            != Some(fingerprint.as_str())
        {
            self.index.clear().map_err(|e| e.to_string())?;
            store
                .set_json("image_index_model", &fingerprint)
                .map_err(|e| e.to_string())?;
        }
        self.prefs
            .set(AI_IMAGE_SEARCH_MODEL, model)
            .map_err(|e| e.to_string())?;
        self.index.set_encoder(Some(engine.clone()));
        *self.engine.lock().unwrap() = Some(engine);
        self.prefs
            .set("ai_image_search_enabled", true)
            .map_err(|e| e.to_string())?;
        if !self.observing.load(Ordering::Acquire) {
            if self.index.uses_system_catalog() {
                self.host
                    .call("systemImageModelsObserve", json!({"enabled":true}))
                    .await?;
            }
            self.observing.store(true, Ordering::Release);
        }
        self.index.start(false).map_err(|e| e.to_string())?;
        self.set_status(ImageSearchStatusType::Ready, String::new());
        if previous.exists() {
            let _ = tokio::fs::remove_dir_all(previous).await;
        }
        Ok(())
    }
    async fn download(&self) -> Result<(), String> {
        use tokio::io::AsyncWriteExt;
        let directory = self.directory.join("download");
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let manifest = plain_inference::models::default_model::manifest();
        let total = manifest.size();
        let mut downloaded = 0u64;
        for (asset, url) in plain_inference::models::default_model::downloads(&manifest) {
            if self.cancelled.load(Ordering::Acquire) {
                return Err("Model download cancelled".into());
            }
            let name = asset.name.as_str();
            let expected = asset.size;
            let mut response = client
                .get(url)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let partial = directory.join(format!("{name}.part"));
            let mut file = tokio::fs::File::create(&partial)
                .await
                .map_err(|e| e.to_string())?;
            let mut size = 0u64;
            while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
                if self.cancelled.load(Ordering::Acquire) {
                    return Err("Model download cancelled".into());
                }
                file.write_all(&bytes).await.map_err(|e| e.to_string())?;
                size += bytes.len() as u64;
                downloaded += bytes.len() as u64;
                if size > expected {
                    return Err(format!("Unexpected model size: {name}"));
                }
                let progress = (downloaded.saturating_mul(100) / total).min(99) as i32;
                let changed = {
                    let mut state = self.state.lock().unwrap();
                    let changed = state.progress != progress;
                    state.progress = progress;
                    changed
                };
                if changed {
                    self.publish();
                }
            }
            file.flush().await.map_err(|e| e.to_string())?;
            drop(file);
            if size != expected {
                return Err(format!("Incomplete model: {name}"));
            }
            tokio::fs::rename(partial, directory.join(name))
                .await
                .map_err(|e| e.to_string())?;
        }
        tokio::fs::write(
            directory.join("LICENSE"),
            include_str!("../../../../plain-inference/src/APACHE-2.0.txt"),
        )
        .await
        .map_err(|e| e.to_string())?;
        tokio::fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string())?;
        self.state.lock().unwrap().progress = 100;
        self.publish();
        Ok(())
    }
    pub async fn search(
        &self,
        text: &str,
        limit: usize,
    ) -> Result<Vec<plain_rs::media::images::embeddings::SearchResult>, String> {
        let _guard = self.operations.lock().await;
        if self.snapshot().status.status != ImageSearchStatusType::Ready {
            return Err("Image search models are unavailable".into());
        }
        if limit > 500 {
            return Err("Image search limit too large".into());
        }
        let engine = self
            .engine
            .lock()
            .unwrap()
            .clone()
            .ok_or("Image search engine unavailable")?;
        let minimum_score = engine.manifest.minimum_score;
        let text = text.to_owned();
        let vector = tokio::task::spawn_blocking(move || engine.text(&text))
            .await
            .map_err(|e| e.to_string())??;
        let bytes = vector
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let encoded = plain_rs::utils::base64::base64_encode(&bytes);
        let index = self.index.clone();
        tokio::task::spawn_blocking(move || index.search_with_score(&encoded, limit, minimum_score))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())
    }
    async fn stop_task(&self) {
        self.cancelled.store(true, Ordering::Release);
        let task = self.task.lock().unwrap().take();
        if let Some(task) = task {
            let _ = task.await;
        }
    }
    async fn pause_index(&self) -> Result<(), String> {
        self.index.cancel().map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(45), self.index.wait_until_idle())
            .await
            .map_err(|_| "Image index cancellation timed out".to_owned())?;
        Ok(())
    }
    async fn close_models(&self) -> Result<(), String> {
        self.cancel_catalog_changes();
        self.pause_index().await?;
        self.index.set_encoder(None);
        *self.engine.lock().unwrap() = None;
        if self.observing.swap(false, Ordering::AcqRel) {
            if self.index.uses_system_catalog() {
                self.host
                    .call("systemImageModelsObserve", json!({"enabled":false}))
                    .await?;
            }
        }
        Ok(())
    }
    pub async fn cancel(&self, disable: bool) -> Result<(), String> {
        let _guard = self.operations.lock().await;
        let active = self
            .task
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|task| !task.is_finished());
        let status = self.snapshot().status.status;
        if !disable
            && (matches!(
                status,
                ImageSearchStatusType::Ready | ImageSearchStatusType::Error
            ) || (!active
                && !matches!(
                    status,
                    ImageSearchStatusType::Downloading | ImageSearchStatusType::Loading
                )))
        {
            return Ok(());
        }
        self.stop_task().await;
        if !disable && self.engine.lock().unwrap().is_some() {
            let engine = self.engine.lock().unwrap().clone().unwrap();
            self.index.set_encoder(Some(engine));
            self.set_status(ImageSearchStatusType::Ready, String::new());
            return Ok(());
        }
        self.close_models().await?;
        if disable {
            self.index.clear().map_err(|e| e.to_string())?;
        }
        self.prefs
            .set("ai_image_search_enabled", false)
            .map_err(|e| e.to_string())?;
        if disable && self.directory.exists() {
            tokio::fs::remove_dir_all(&self.directory)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            for name in ["download", "incoming"] {
                let path = self.directory.join(name);
                if path.exists() {
                    tokio::fs::remove_dir_all(path)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        self.state.lock().unwrap().progress = 0;
        self.set_status(ImageSearchStatusType::Unavailable, String::new());
        Ok(())
    }
    pub async fn shutdown(&self) {
        let _guard = self.operations.lock().await;
        self.stop_task().await;
        if let Err(error) = self.close_models().await {
            log::warn!("Image model shutdown failed: {error}");
        }
        self.set_status(ImageSearchStatusType::Unavailable, String::new());
    }
    pub fn follow(self: &Arc<Self>, mut stop: tokio::sync::watch::Receiver<bool>) {
        self.publish();
        let runtime = self.clone();
        let mut catalog_events = self.events.subscribe();
        tokio::spawn(async move {
            if !*stop.borrow() {
                tokio::select! {
                    _ = stop.changed() => {},
                    result = runtime.enable(true) => {
                        if let Err(error) = result { runtime.report_error(error); }
                    },
                }
            }
            while !*stop.borrow() {
                tokio::select! {
                    _ = stop.changed() => break,
                    event = catalog_events.recv() => match event {
                        Ok(event) if event.event_type == "IMAGE_INDEX_UPDATED" => runtime.publish(),
                        Ok(event) if event.event_type == plain_rs::events::MEDIA_LIBRARY_UPDATED && plain_rs::services::files::catalog_events::includes(&event.payload, plain_rs::enums::DataType::Image) => runtime.catalog_changed(),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { runtime.catalog_changed(); runtime.publish(); },
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        _ => continue,
                    },
                }
            }
            runtime.shutdown().await;
        });
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/images/models.rs"]
mod tests;
