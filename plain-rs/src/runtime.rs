use plain_rs::{db::Db, events::WsEvent};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
};
#[derive(Clone)]
pub struct Runtime {
    pub ble_advertising: Arc<plain_rs::services::discovery::ble_advertising::Runtime>,
    _data_directory: Arc<plain_rs::data_paths::directory::DataDirectory>,
    pub cache: plain_rs::cache_paths::CachePaths,
    pub cache_changes: Arc<tokio::sync::RwLock<()>>,
    pub remote_cache: Arc<plain_rs::services::files::remote_cache::RemoteCache>,
    pub logs: Arc<plain_rs::services::logs::Logs>,
    pub device: Arc<plain_rs::services::device::Status>,
    pub browser: Arc<plain_rs::services::files::browser::Runtime>,
    pub file_platform: Arc<plain_rs::services::files::platform::FilePlatform>,
    pub feeds: Arc<plain_rs::services::feeds::SyncService>,
    pomodoro_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    external_index_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    pub update_checks: Arc<plain_rs::services::settings::updates::Runtime>,
    pub duration_updates: Arc<plain_rs::services::files::duration_updates::Runtime>,

    pub duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    pub store: Arc<plain_rs::kv::LazyDb>,
    pub http_server: Arc<plain_rs::services::http_status::Runtime>,
    pub web_root: Arc<std::sync::RwLock<Option<PathBuf>>>,
    pub tls_identity: Arc<std::sync::RwLock<plain_rs::services::tls_identity::IdentitySource>>,
    pub aware: Arc<plain_rs::services::peers::aware::Runtime>,
    pub peer_status: Arc<plain_rs::services::peers::status_runtime::Runtime>,
    pub management: Arc<plain_rs::services::peers::management::Runtime>,
    pub clipboard_changes: Arc<plain_rs::services::clipboard::Changes>,
    pub network_revision: watch::Sender<u64>,
    pub mdns: Arc<plain_rs::services::discovery::mdns_runtime::Runtime>,
    pub archives: Arc<plain_rs::services::files::archives::Archives>,
    pub upload_chunks: Arc<plain_rs::services::files::upload_chunks::UploadChunks>,
    pub files: Arc<plain_rs::services::files::file_tasks::FileTasks>,
    #[cfg(feature = "runtime-media")]
    pub media_scans: Arc<plain_rs::services::files::media_scans::MediaScans>,
    /// The live media watcher. Held so `stop_core` can release it; the
    /// catalog replaces it rather than stacking a second watch.
    #[cfg(feature = "runtime-media")]
    pub media_watch: std::sync::Arc<
        std::sync::Mutex<Option<plain_rs::services::files::media_watch::WatcherHandle>>,
    >,
    pub host: Arc<plain_rs::host::bridge::Host>,
    pub thumbnails: Arc<plain_rs::services::images::thumbnails::Thumbnails>,
    pub db: Arc<Db>,
    pub lan: Arc<plain_rs::services::peers::peer_lan::Lan>,
    pub transport: Arc<plain_rs::transport::router::Router>,
    pub transport_delivery: Arc<std::sync::Mutex<bool>>,
    pub previews: Arc<plain_rs::services::link_preview::Schedule>,
    pub prewarmer: Arc<plain_rs::chat::attachments::prewarm::Prewarmer>,
    pub shared_batches: Arc<plain_rs::services::peers::shared_batch::Runtime>,
    pub download_runtime: Arc<plain_rs::services::chat::download_queue::Runtime>,
    pub downloads: Arc<plain_rs::chat::attachments::download_queue::Queue>,
    pub delivery: Arc<plain_rs::chat::delivery::Delivery>,
    pub channel_delivery: Arc<tokio::sync::Semaphore>,
    pub ble_transport: Arc<plain_rs::services::peers::ble_transport::Runtime>,
    pub ble_pairing: Arc<plain_rs::services::discovery::ble_pairing::Pairings>,
    pub pairing_runtime: Arc<plain_rs::services::discovery::pairing_runtime::Runtime>,
    pub nearby_devices: Arc<plain_rs::discovery::nearby_devices::Devices>,
    pub ble_scans: Arc<plain_rs::discovery::nearby_scan::Scans>,
    pub ble_scan: Arc<plain_rs::services::discovery::ble_scan::Runtime>,
    pub pairing: Arc<plain_rs::peers::pairing::sessions::Sessions>,
    pub prefs: Arc<plain_rs::prefs::Prefs>,
    pub dlna: Arc<plain_rs::services::casting::receiver::receiver_engine::DlnaEngine>,
    pub sms_sends: Arc<plain_rs::services::sms::send_runtime::Runtime>,
    pub image_models: Arc<plain_rs::services::images::models::Runtime>,
    pub cast: Arc<plain_rs::services::casting::dlna_sender_runtime::Runtime>,
    pub audio: Arc<plain_rs::services::audio::host::Audio>,
    pub directory: std::path::PathBuf,
    pub token: Arc<str>,
    pub temp_values: plain_rs::services::temp_values::TempValues,
    pub events: broadcast::Sender<WsEvent>,
    pub stop: watch::Receiver<bool>,
    pub media_info: Arc<plain_rs::services::files::info::Info>,
    pub resources: Arc<plain_rs::host::resources::NativeResources>,
    pub sms: Arc<plain_rs::services::sms::mutations::Mutations>,
    pub app_files: Arc<plain_rs::files::app_files::FileStore>,
    pub shares: Arc<plain_rs::files::shares::Service>,
    pub media_actions: Arc<plain_rs::services::files::media_actions::MediaActions>,
    pub index: Arc<plain_rs::services::images::index::ImageIndex>,
    pub pomodoro: Arc<plain_rs::services::pomodoro::Service>,
    pub feed_assets: Arc<plain_rs::services::feeds::FeedAssets>,
    pub image_updates: Arc<plain_rs::media::images::editor::Updates>,
    pub uploads: Arc<plain_rs::files::uploads::Runtime>,
    pub mms: Arc<plain_rs::services::sms::mms::Runtime>,
    pub clients: Arc<dyn plain_rs::host::client_connections::ClientConnections>,
}
impl Runtime {
    pub fn open(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        host: Arc<plain_rs::host::bridge::Host>,
        clients: Arc<dyn plain_rs::host::client_connections::ClientConnections>,
    ) -> Result<(Self, watch::Sender<bool>), String> {
        if plain_rs::utils::base64::base64_decode(token).len() != 32 {
            return Err("token must contain 32 random bytes".into());
        }
        if !directory.is_absolute() {
            return Err("Absolute core data directory required".into());
        }
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let cache_directory = match host.startup().cache_dir.as_deref() {
            Some(path) => PathBuf::from(path),
            None => plain_rs::cache_paths::system::resolve(directory)?,
        };
        let data_directory =
            plain_rs::data_paths::directory::DataDirectory::open(directory, &cache_directory)?;
        let paths = plain_rs::data_paths::DataPaths::new(directory);
        let db = Arc::new(Db::open(&paths.database()).map_err(|e| e.to_string())?);
        let (events, _) = broadcast::channel(256);
        Self::from_storage(
            directory,
            token,
            prefs,
            host,
            clients,
            db,
            data_directory,
            events,
        )
    }
    pub fn from_storage(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        host: Arc<plain_rs::host::bridge::Host>,
        clients: Arc<dyn plain_rs::host::client_connections::ClientConnections>,
        db: Arc<Db>,
        data_directory: Arc<plain_rs::data_paths::directory::DataDirectory>,
        events: broadcast::Sender<WsEvent>,
    ) -> Result<(Self, watch::Sender<bool>), String> {
        let directory = directory.to_path_buf();
        let token = token.to_owned();
        plain_rs::prefs::security::bootstrap(&prefs).map_err(|error| error.to_string())?;
        log::set_max_level(
            plain_rs::prefs::security::log_level(&prefs).map_err(|error| error.to_string())?,
        );
        if prefs
            .get::<String>(plain_rs::prefs::pref_keys::DEVICE_NAME)
            .map_err(|error| error.to_string())?
            .is_none_or(|name| name.trim().is_empty())
            && !host.startup().name.trim().is_empty()
        {
            plain_rs::prefs::settings::patch(
                &prefs,
                plain_rs::prefs::settings::SettingsPatch {
                    device_name: Some(host.startup().name.clone()),
                    ..Default::default()
                },
            )
            .map_err(|error| error.to_string())?;
        }
        plain_rs::prefs::identity::ensure_identity(&prefs);
        let (stop, receiver) = watch::channel(false);
        let lan =
            Arc::new(plain_rs::services::peers::peer_lan::Lan::new().map_err(|e| e.to_string())?);
        let cache = data_directory.cache_paths().clone();

        let path = plain_rs::data_paths::DataPaths::new(&directory).database();
        let store = Arc::new(plain_rs::kv::LazyDb::from_open(path, db.key_values()));
        let duration_cache = Arc::new(plain_rs::media::duration_cache::DurationCache::with_store(
            store.clone(),
        ));
        let resources = Arc::new(plain_rs::host::resources::NativeResources::new(
            host.clone(),
        ));
        let media_info = plain_rs::services::files::info::Info::new(
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        );
        let duration_updates = plain_rs::services::files::duration_updates::Runtime::new(
            duration_cache.clone(),
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        );
        let audio = Arc::new(plain_rs::services::audio::host::Audio::new(
            db.clone(),
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        ));
        let index = Arc::new(plain_rs::services::images::index::ImageIndex::new(
            db.clone(),
            host.clone(),
            events.clone(),
            resources.clone(),
            cache.root().to_owned(),
        ));
        #[cfg(feature = "runtime-media")]
        let media_scans = Arc::new(plain_rs::services::files::media_scans::MediaScans::new(
            events.clone(),
            cache.root().to_owned(),
        ));
        let file_platform = Arc::new(plain_rs::services::files::platform::FilePlatform::new(
            host.clone(),
            db.clone(),
            directory.clone(),
            cache.root().to_owned(),
            events.clone(),
            #[cfg(feature = "runtime-media")]
            media_scans.clone(),
        ));
        let files = Arc::new(plain_rs::services::files::file_tasks::FileTasks::new(
            file_platform.clone(),
            events.clone(),
            audio.clone(),
            index.clone(),
            prefs.clone(),
            duration_cache.clone(),
        ));
        let app_files = plain_rs::files::app_files::FileStore::new(db.clone(), directory.clone());
        let shares = plain_rs::files::shares::Service::new(db.clone(), prefs.clone());
        let sms = plain_rs::services::sms::mutations::Mutations::new(
            db.clone(),
            host.clone(),
            prefs.clone(),
        );
        let sms_sends =
            plain_rs::services::sms::send_runtime::Runtime::new(store.clone(), events.clone());
        let logs = Arc::new(plain_rs::services::logs::Logs::new(&directory));
        let remote_cache = Arc::new(plain_rs::services::files::remote_cache::RemoteCache::new(
            cache.root().to_owned(),
        ));
        let feed_assets = Arc::new(plain_rs::services::feeds::FeedAssets::new(
            db.clone(),
            directory.clone(),
        ));
        let feeds = plain_rs::services::feeds::SyncService::new(
            db.clone(),
            events.clone(),
            host.clone(),
            Some(feed_assets.clone()),
        );
        let pomodoro =
            plain_rs::services::pomodoro::Service::new(db.clone(), prefs.clone(), events.clone());
        let image_updates = plain_rs::media::images::editor::Updates::new(events.clone());
        let uploads = Arc::new(plain_rs::files::uploads::Runtime::with_completion({
            let platform = file_platform.clone();
            move |path| {
                let platform = platform.clone();
                Box::pin(async move { platform.scan(&[path.to_string_lossy().into_owned()]).await })
            }
        }));
        let browser = plain_rs::services::files::browser::Runtime::new(cache.root().to_owned());
        let device = Arc::new(plain_rs::services::device::Status::new(
            directory.clone(),
            &host,
        ));
        let media_actions = Arc::new(plain_rs::services::files::media_actions::MediaActions::new(
            #[cfg(feature = "runtime-media")]
            media_scans.clone(),
            file_platform.clone(),
            db.clone(),
            host.clone(),
            audio.clone(),
            index.clone(),
            prefs.clone(),
            events.clone(),
            duration_cache.clone(),
        ));
        device.fixed_info()?;
        let mms = plain_rs::services::sms::mms::Runtime::new(
            host.clone(),
            db.clone(),
            directory.clone(),
            store.clone(),
            events.clone(),
        );
        let image_models = plain_rs::services::images::models::Runtime::new(
            directory.clone(),
            host.clone(),
            prefs.clone(),
            store.clone(),
            index.clone(),
            events.clone(),
        );
        let attachments =
            Arc::new(plain_rs::chat::attachments::attachment_imports::Imports::default());
        let downloads = Arc::new(plain_rs::chat::attachments::download_queue::Queue::new(
            (*db).clone(),
            directory.clone(),
            attachments.clone(),
        ));
        let preview_events = events.clone();
        let previews = Arc::new(plain_rs::services::link_preview::Schedule::new(
            (*db).clone(),
            directory.clone(),
            Arc::new(move |row| {
                let _ = preview_events.send(WsEvent::broadcast(
                    plain_rs::chat::events::WS_MESSAGE_UPDATED,
                    serde_json::json!([plain_rs::chat::service::chat_to_json(&row)]).to_string(),
                ));
            }),
            receiver.clone(),
        ));
        let dlna =
            Arc::new(plain_rs::services::casting::receiver::receiver_engine::DlnaEngine::new());
        let state = Self {
            ble_advertising: Arc::new(
                plain_rs::services::discovery::ble_advertising::Runtime::default(),
            ),
            cache: data_directory.cache_paths().clone(),
            _data_directory: data_directory,
            logs: logs.clone(),
            device: device.clone(),
            file_platform: file_platform.clone(),
            browser: browser.clone(),
            cache_changes: Arc::new(tokio::sync::RwLock::new(())),
            remote_cache: remote_cache.clone(),
            feeds: feeds.clone(),
            pomodoro_task: Arc::new(parking_lot::Mutex::new(None)),
            external_index_task: Default::default(),
            update_checks: Arc::new(plain_rs::services::settings::updates::Runtime::default()),

            duration_cache: duration_cache.clone(),
            duration_updates: duration_updates.clone(),
            store,
            #[cfg(feature = "runtime-media")]
            media_scans: media_scans.clone(),
            #[cfg(feature = "runtime-media")]
            media_watch: std::sync::Arc::new(std::sync::Mutex::new(None)),
            lan,
            aware: Arc::new(plain_rs::services::peers::aware::Runtime::default()),
            peer_status: Arc::new(plain_rs::services::peers::status_runtime::Runtime::default()),
            management: Arc::new(plain_rs::services::peers::management::Runtime::default()),
            clipboard_changes: Arc::new(plain_rs::services::clipboard::Changes::default()),
            network_revision: watch::channel(0).0,
            mdns: Arc::new(plain_rs::services::discovery::mdns_runtime::Runtime::default()),
            transport: Arc::new(plain_rs::transport::router::Router::default()),
            transport_delivery: Arc::new(std::sync::Mutex::new(false)),
            previews,
            prewarmer: Arc::new(plain_rs::chat::attachments::prewarm::Prewarmer::default()),
            shared_batches: Arc::new(plain_rs::services::peers::shared_batch::Runtime::default()),
            download_runtime: Arc::new(plain_rs::services::chat::download_queue::Runtime::default()),
            downloads,
            delivery: Arc::new(plain_rs::chat::delivery::Delivery::new((*db).clone())),
            channel_delivery: Arc::new(tokio::sync::Semaphore::new(4)),
            ble_transport: Arc::new(plain_rs::services::peers::ble_transport::Runtime::default()),
            ble_pairing: Arc::new(plain_rs::services::discovery::ble_pairing::Pairings::default()),
            pairing_runtime: Arc::new(
                plain_rs::services::discovery::pairing_runtime::Runtime::default(),
            ),
            nearby_devices: Arc::new(plain_rs::discovery::nearby_devices::Devices::default()),
            ble_scans: Arc::new(plain_rs::discovery::nearby_scan::Scans::default()),
            ble_scan: Arc::new(plain_rs::services::discovery::ble_scan::Runtime::default()),
            pairing: Arc::new(plain_rs::peers::pairing::sessions::Sessions::default()),
            dlna,
            archives: Arc::new(plain_rs::services::files::archives::Archives::new(
                directory.join("archives"),
            )),
            upload_chunks: Arc::new(plain_rs::services::files::upload_chunks::UploadChunks::new(
                directory.join("uploads"),
            )),
            files: files,
            audio: audio,
            host,
            db,
            prefs,
            sms_sends: sms_sends.clone(),
            image_models: image_models.clone(),
            cast: Arc::new(plain_rs::services::casting::dlna_sender_runtime::Runtime::default()),
            directory: directory.clone(),
            http_server: Arc::new(plain_rs::services::http_status::Runtime::default()),
            web_root: Arc::new(std::sync::RwLock::new(None)),
            tls_identity: Arc::new(std::sync::RwLock::new(
                plain_rs::services::tls_identity::IdentitySource::Json(
                    plain_rs::data_paths::DataPaths::new(&directory).tls_identity(),
                ),
            )),
            token: Arc::from(token.as_str()),
            temp_values: Default::default(),
            events,
            stop: receiver.clone(),
            media_info,
            resources,
            sms,
            app_files,
            shares,
            media_actions,
            index,
            pomodoro,
            feed_assets,
            image_updates,
            uploads,
            mms,
            clients,
            thumbnails: Arc::new(plain_rs::services::images::thumbnails::Thumbnails::default()),
        };
        state
            .mms
            .set_resources(state.resources.clone(), receiver.clone());
        plain_rs::services::feeds::sync_service::schedule::start(state.clone());
        plain_rs::services::chat::download_queue::start(state.clone());
        plain_rs::services::peers::shared_batch::start(state.clone());
        plain_rs::services::peers::aware::start(state.clone());
        *state.external_index_task.lock() = Some(plain_rs::services::files::external_index::start(
            state.clone(),
        ));
        state
            .management
            .start(state.clone())
            .map_err(|error| error.to_string())?;
        plain_rs::services::discovery::pairing_timeout::start(
            state.pairing.clone(),
            state.events.clone(),
            receiver.clone(),
        );
        plain_rs::services::discovery::nearby_devices::start(state.clone());
        plain_rs::services::discovery::ble_scan::start(state.clone());
        *state.pomodoro_task.lock() = Some(state.pomodoro.follow(receiver.clone()));
        image_models.follow(receiver.clone());
        state.sms_sends.follow(receiver.clone());
        state.mms.follow(receiver.clone());
        plain_rs::services::casting::receiver::control::follow_changes(&state);
        let mdns = state.mdns.clone();
        let mdns_host = state.host.clone();
        let mut mdns_stop = receiver.clone();
        tokio::spawn(async move {
            if !*mdns_stop.borrow() {
                let _ = mdns_stop.changed().await;
            }
            mdns.close(&mdns_host).await;
        });
        Ok((state, stop))
    }
    pub fn native_url_token(&self) -> Result<String, String> {
        self.native_client_id()?;
        self.prefs
            .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
            .map_err(|error| error.to_string())?
            .filter(|token| !token.is_empty())
            .ok_or_else(|| "native URL token unavailable".into())
    }

    pub fn native_client_id(&self) -> Result<String, String> {
        if *self.stop.borrow() || self.stop.has_changed().is_err() {
            return Err("Rust core stopped".into());
        }
        self.prefs
            .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)
            .map_err(|error| error.to_string())?
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "native client identity unavailable".into())
    }

    pub async fn public_ports(&self) -> Option<(u16, u16)> {
        let state = self.http_server.snapshot();
        (state.http_port != 0).then_some((state.http_port as u16, state.https_port as u16))
    }
    pub async fn shutdown(&self, stop: &watch::Sender<bool>) {
        self.shared_batches.shutdown(&self.directory).await;
        let external_index = self.external_index_task.lock().take();
        if let Some(task) = external_index {
            task.abort();
            let _ = task.await;
        }
        self.feeds.shutdown().await;
        self.download_runtime.shutdown().await;
        self.update_checks.download.shutdown().await;
        self.mms.shutdown().await;
        self.image_models.shutdown().await;
        self.duration_updates.shutdown().await;
        if self.host.connected() && self.host.needs_socket_cleanup() {
            let _ = self
                .host
                .call("peerTransportSocketCloseAll", serde_json::json!({}))
                .await;
        }
        #[cfg(feature = "runtime-media")]
        {
            // The watcher writes into the catalog, so it has to stop before
            // the scan state it feeds is closed.
            if let Ok(mut slot) = self.media_watch.lock() {
                slot.take();
            }
            if let Err(error) = self.media_scans.close() {
                log::error!("Media scan cancellation failed: {error}");
            }
        }
        let _ = stop.send(true);
        self.management.shutdown().await;
        let pomodoro_task = self.pomodoro_task.lock().take();
        if let Some(task) = pomodoro_task {
            if let Err(error) = task.await {
                log::warn!("Pomodoro shutdown: {error}");
            }
        }
        #[cfg(feature = "runtime-media")]
        if let Err(error) = self.media_scans.wait_background().await {
            log::error!("Media scan task shutdown failed: {error}");
        }
        self.mdns.close(&self.host).await;
    }
}
