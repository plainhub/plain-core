use crate::infrastructure::db::Db;
use crate::infrastructure::events::WsEvent;
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
    pub ble_advertising: Arc<crate::features::services::discovery::ble_advertising::Runtime>,
    _data_directory: Arc<crate::infrastructure::data_paths::directory::DataDirectory>,
    pub cache: crate::infrastructure::cache_paths::CachePaths,
    pub cache_changes: Arc<tokio::sync::RwLock<()>>,
    pub remote_cache: Arc<crate::features::services::files::remote_cache::RemoteCache>,
    pub logs: Arc<crate::infrastructure::logs::Logs>,
    pub device: Arc<crate::features::services::device::Status>,
    pub(crate) browser: Arc<crate::features::services::files::browser::Runtime>,
    pub file_platform: Arc<crate::features::services::files::platform::FilePlatform>,
    pub feeds: Arc<crate::features::services::feeds::SyncService>,
    pomodoro_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    buffered_writes_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    external_index_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    pub update_checks: Arc<crate::features::services::settings::updates::Runtime>,
    pub duration_updates: Arc<crate::features::services::files::duration_updates::Runtime>,

    pub duration_cache: Arc<crate::features::media::duration_cache::DurationCache>,
    pub store: Arc<plain_libs::kv::LazyDb>,
    pub http_server: Arc<crate::features::services::http_status::Runtime>,
    pub web_root: Arc<std::sync::RwLock<Option<PathBuf>>>,
    pub tls_identity: Arc<std::sync::RwLock<crate::infrastructure::tls_identity::IdentitySource>>,
    pub aware: Arc<crate::features::services::peers::aware::Runtime>,
    pub peer_status: Arc<crate::features::services::peers::status_runtime::Runtime>,
    pub management: Arc<crate::features::services::peers::management::Runtime>,
    pub clipboard_changes: Arc<crate::features::services::clipboard::Changes>,
    pub network_revision: watch::Sender<u64>,
    pub mdns: Arc<crate::features::services::discovery::mdns_runtime::Runtime>,
    pub archives: Arc<crate::features::services::files::archives::Archives>,
    pub upload_chunks: Arc<crate::features::services::files::upload_chunks::UploadChunks>,
    pub files: Arc<crate::features::services::files::file_tasks::FileTasks>,
    #[cfg(feature = "runtime-media")]
    pub media_scans: Arc<crate::features::services::files::media_scans::MediaScans>,
    /// The live media watcher. Held so `stop_core` can release it; the
    /// catalog replaces it rather than stacking a second watch.
    #[cfg(feature = "runtime-media")]
    pub media_watch: std::sync::Arc<
        std::sync::Mutex<Option<crate::features::services::files::media_watch::WatcherHandle>>,
    >,
    pub host: Arc<crate::infrastructure::host::bridge::Host>,
    pub thumbnails: Arc<crate::features::services::images::thumbnails::Thumbnails>,
    pub db: Arc<Db>,
    pub lan: Arc<crate::features::services::peers::peer_lan::Lan>,
    pub transport: Arc<crate::infrastructure::transport::router::Router>,
    pub(crate) transport_delivery: Arc<std::sync::Mutex<bool>>,
    pub previews: Arc<crate::features::services::link_preview::Schedule>,
    pub prewarmer: Arc<crate::features::chat::attachments::prewarm::Prewarmer>,
    pub shared_batches: Arc<crate::features::services::peers::shared_batch::Runtime>,
    pub(crate) download_runtime: Arc<crate::features::services::chat::download_queue::Runtime>,
    pub downloads: Arc<crate::features::chat::attachments::download_queue::Queue>,
    pub(crate) delivery: Arc<crate::features::chat::delivery::Delivery>,
    pub(crate) channel_delivery: Arc<tokio::sync::Semaphore>,
    pub ble_transport: Arc<crate::features::services::peers::ble_transport::Runtime>,
    pub(crate) ble_pairing: Arc<crate::features::services::discovery::ble_pairing::Pairings>,
    pub pairing_runtime: Arc<crate::features::services::discovery::pairing_runtime::Runtime>,
    pub nearby_devices: Arc<crate::features::discovery::nearby_devices::Devices>,
    pub ble_scans: Arc<crate::features::discovery::nearby_scan::Scans>,
    pub(crate) ble_scan: Arc<crate::features::services::discovery::ble_scan::Runtime>,
    pub pairing: Arc<crate::features::peers::pairing::sessions::Sessions>,
    pub prefs: Arc<crate::features::prefs::Prefs>,
    pub dlna: Arc<crate::features::services::casting::receiver::receiver_engine::DlnaEngine>,
    pub sms_sends: Arc<crate::features::services::sms::send_runtime::Runtime>,
    pub image_models: Arc<crate::features::services::images::models::Runtime>,
    pub cast: Arc<crate::features::services::casting::dlna_sender_runtime::Runtime>,
    pub audio: Arc<crate::features::services::audio::host::Audio>,
    pub directory: std::path::PathBuf,
    pub token: Arc<str>,
    pub temp_values: crate::infrastructure::temp_values::TempValues,
    pub events: broadcast::Sender<WsEvent>,
    pub stop: watch::Receiver<bool>,
    pub media_info: Arc<crate::features::services::files::info::Info>,
    pub resources: Arc<crate::infrastructure::host::resources::NativeResources>,
    pub sms: Arc<crate::features::services::sms::mutations::Mutations>,
    pub app_files: Arc<crate::features::files::app_files::FileStore>,
    pub shares: Arc<crate::features::files::shares::Service>,
    pub media_actions: Arc<crate::features::services::files::media_actions::MediaActions>,
    pub index: Arc<crate::features::services::images::index::ImageIndex>,
    pub pomodoro: Arc<crate::features::services::pomodoro::Service>,
    pub feed_assets: Arc<crate::features::services::feeds::FeedAssets>,
    pub image_updates: Arc<crate::features::media::images::editor::Updates>,
    pub uploads: Arc<crate::features::files::uploads::Runtime>,
    pub mms: Arc<crate::features::services::sms::mms::Runtime>,
    pub(crate) clients: Arc<dyn crate::infrastructure::host::client_connections::ClientConnections>,
}
impl Runtime {
    pub fn open(
        directory: &Path,
        token: &str,
        prefs: Arc<crate::features::prefs::Prefs>,
        host: Arc<crate::infrastructure::host::bridge::Host>,
        clients: Arc<dyn crate::infrastructure::host::client_connections::ClientConnections>,
    ) -> Result<(Self, watch::Sender<bool>), String> {
        if plain_libs::utils::base64::base64_decode(token).len() != 32 {
            return Err("token must contain 32 random bytes".into());
        }
        if !directory.is_absolute() {
            return Err("Absolute core data directory required".into());
        }
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let cache_directory = match host.startup().cache_dir.as_deref() {
            Some(path) => PathBuf::from(path),
            None => crate::infrastructure::cache_paths::system::resolve(directory)?,
        };
        let data_directory = crate::infrastructure::data_paths::directory::DataDirectory::open(
            directory,
            &cache_directory,
        )?;
        let paths = crate::infrastructure::data_paths::DataPaths::new(directory);
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
        prefs: Arc<crate::features::prefs::Prefs>,
        host: Arc<crate::infrastructure::host::bridge::Host>,
        clients: Arc<dyn crate::infrastructure::host::client_connections::ClientConnections>,
        db: Arc<Db>,
        data_directory: Arc<crate::infrastructure::data_paths::directory::DataDirectory>,
        events: broadcast::Sender<WsEvent>,
    ) -> Result<(Self, watch::Sender<bool>), String> {
        let directory = directory.to_path_buf();
        let token = token.to_owned();
        crate::features::prefs::security::bootstrap(&prefs).map_err(|error| error.to_string())?;
        log::set_max_level(
            crate::features::prefs::security::log_level(&prefs)
                .map_err(|error| error.to_string())?,
        );
        if prefs
            .get::<String>(crate::features::prefs::pref_keys::DEVICE_NAME)
            .map_err(|error| error.to_string())?
            .is_none_or(|name| name.trim().is_empty())
            && !host.startup().name.trim().is_empty()
        {
            crate::features::prefs::settings::patch(
                &prefs,
                crate::features::prefs::settings::SettingsPatch {
                    device_name: Some(host.startup().name.clone()),
                    ..Default::default()
                },
            )
            .map_err(|error| error.to_string())?;
        }
        let identity = crate::features::prefs::identity::ensure_identity(&prefs);
        crate::features::sessions::SessionStore::new(&db)
            .upsert(crate::features::sessions::SessionInfo {
                client_id: identity.client_id,
                token: token.clone(),
                client_name: identity.device_name,
                app_version: host.startup().app_version.clone(),
                os_name: host.startup().os_name.clone(),
                os_version: host.startup().os_version.clone(),
                ..Default::default()
            })
            .map_err(|error| error.to_string())?;
        let (stop, receiver) = watch::channel(false);
        let lan = Arc::new(
            crate::features::services::peers::peer_lan::Lan::new().map_err(|e| e.to_string())?,
        );
        let cache = data_directory.cache_paths().clone();

        let path = crate::infrastructure::data_paths::DataPaths::new(&directory).database();
        let store = Arc::new(plain_libs::kv::LazyDb::from_open(path, db.key_values()));
        let duration_cache = Arc::new(
            crate::features::media::duration_cache::DurationCache::with_store(store.clone()),
        );
        let resources =
            Arc::new(crate::infrastructure::host::resources::NativeResources::new(host.clone()));
        let media_info = crate::features::services::files::info::Info::new(
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        );
        let duration_updates = crate::features::services::files::duration_updates::Runtime::new(
            duration_cache.clone(),
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        );
        let audio = Arc::new(crate::features::services::audio::host::Audio::new(
            db.clone(),
            host.clone(),
            resources.clone(),
            cache.root().to_owned(),
        ));
        let index = Arc::new(crate::features::services::images::index::ImageIndex::new(
            db.clone(),
            host.clone(),
            events.clone(),
            resources.clone(),
            cache.root().to_owned(),
        ));
        #[cfg(feature = "runtime-media")]
        let media_scans = Arc::new(
            crate::features::services::files::media_scans::MediaScans::new(
                events.clone(),
                cache.root().to_owned(),
            ),
        );
        let file_platform = Arc::new(
            crate::features::services::files::platform::FilePlatform::new(
                host.clone(),
                db.clone(),
                directory.clone(),
                cache.root().to_owned(),
                #[cfg(not(feature = "runtime-media"))]
                events.clone(),
                #[cfg(feature = "runtime-media")]
                media_scans.clone(),
            ),
        );
        let files = Arc::new(
            crate::features::services::files::file_tasks::FileTasks::new(
                file_platform.clone(),
                events.clone(),
                audio.clone(),
                index.clone(),
                prefs.clone(),
                duration_cache.clone(),
            ),
        );
        let app_files =
            crate::features::files::app_files::FileStore::new(db.clone(), directory.clone());
        let shares = crate::features::files::shares::Service::new(db.clone(), prefs.clone());
        let sms = crate::features::services::sms::mutations::Mutations::new(
            db.clone(),
            host.clone(),
            prefs.clone(),
        );
        let sms_sends = crate::features::services::sms::send_runtime::Runtime::new(
            store.clone(),
            events.clone(),
        );
        let logs = Arc::new(crate::infrastructure::logs::Logs::new(&directory));
        let remote_cache = Arc::new(
            crate::features::services::files::remote_cache::RemoteCache::new(
                cache.root().to_owned(),
            ),
        );
        let feed_assets = Arc::new(crate::features::services::feeds::FeedAssets::new(
            db.clone(),
            directory.clone(),
        ));
        let feeds = crate::features::services::feeds::SyncService::new(
            db.clone(),
            events.clone(),
            host.clone(),
            Some(feed_assets.clone()),
        );
        let pomodoro = crate::features::services::pomodoro::Service::new(
            db.clone(),
            prefs.clone(),
            events.clone(),
        );
        let image_updates = crate::features::media::images::editor::Updates::new(events.clone());
        let uploads = Arc::new(crate::features::files::uploads::Runtime::with_completion({
            let platform = file_platform.clone();
            move |path| {
                let platform = platform.clone();
                Box::pin(async move {
                    platform
                        .scan(&[path.to_string_lossy().into_owned()])
                        .await
                        .map(|_| ())
                })
            }
        }));
        let browser =
            crate::features::services::files::browser::Runtime::new(cache.root().to_owned());
        let device = Arc::new(crate::features::services::device::Status::new(
            directory.clone(),
            &host,
        ));
        let media_actions = Arc::new(
            crate::features::services::files::media_actions::MediaActions::new(
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
            ),
        );
        device.fixed_info()?;
        let mms = crate::features::services::sms::mms::Runtime::new(
            host.clone(),
            db.clone(),
            directory.clone(),
            store.clone(),
            events.clone(),
        );
        let image_models = crate::features::services::images::models::Runtime::new(
            directory.clone(),
            host.clone(),
            prefs.clone(),
            store.clone(),
            index.clone(),
            events.clone(),
        );
        let attachments =
            Arc::new(crate::features::chat::attachments::attachment_imports::Imports::default());
        let downloads = Arc::new(
            crate::features::chat::attachments::download_queue::Queue::new(
                (*db).clone(),
                directory.clone(),
                attachments.clone(),
            ),
        );
        let preview_events = events.clone();
        let previews = Arc::new(crate::features::services::link_preview::Schedule::new(
            (*db).clone(),
            directory.clone(),
            Arc::new(move |row| {
                let _ = preview_events.send(WsEvent::broadcast(
                    crate::features::chat::events::WS_MESSAGE_UPDATED,
                    plain_libs::serde_json::json!([crate::features::chat::service::chat_to_json(&row)])
                        .to_string(),
                ));
            }),
            receiver.clone(),
        ));
        let dlna = Arc::new(
            crate::features::services::casting::receiver::receiver_engine::DlnaEngine::new(),
        );
        use crate::features::services::images::thumbnails::Thumbnails;
        let thumbnails = Thumbnails::new();
        let state = Self {
            ble_advertising: Arc::new(
                crate::features::services::discovery::ble_advertising::Runtime::default(),
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
            buffered_writes_task: Default::default(),
            external_index_task: Default::default(),
            update_checks: Arc::new(
                crate::features::services::settings::updates::Runtime::default(),
            ),

            duration_cache: duration_cache.clone(),
            duration_updates: duration_updates.clone(),
            store,
            #[cfg(feature = "runtime-media")]
            media_scans: media_scans.clone(),
            #[cfg(feature = "runtime-media")]
            media_watch: std::sync::Arc::new(std::sync::Mutex::new(None)),
            lan,
            aware: Arc::new(crate::features::services::peers::aware::Runtime::default()),
            peer_status: Arc::new(
                crate::features::services::peers::status_runtime::Runtime::default(),
            ),
            management: Arc::new(crate::features::services::peers::management::Runtime::default()),
            clipboard_changes: Arc::new(crate::features::services::clipboard::Changes::default()),
            network_revision: watch::channel(0).0,
            mdns: Arc::new(crate::features::services::discovery::mdns_runtime::Runtime::default()),
            transport: Arc::new(crate::infrastructure::transport::router::Router::default()),
            transport_delivery: Arc::new(std::sync::Mutex::new(false)),
            previews,
            prewarmer: Arc::new(crate::features::chat::attachments::prewarm::Prewarmer::default()),
            shared_batches: Arc::new(
                crate::features::services::peers::shared_batch::Runtime::default(),
            ),
            download_runtime: Arc::new(
                crate::features::services::chat::download_queue::Runtime::default(),
            ),
            downloads,
            delivery: Arc::new(crate::features::chat::delivery::Delivery::new(
                (*db).clone(),
            )),
            channel_delivery: Arc::new(tokio::sync::Semaphore::new(4)),
            ble_transport: Arc::new(
                crate::features::services::peers::ble_transport::Runtime::default(),
            ),
            ble_pairing: Arc::new(
                crate::features::services::discovery::ble_pairing::Pairings::default(),
            ),
            pairing_runtime: Arc::new(
                crate::features::services::discovery::pairing_runtime::Runtime::default(),
            ),
            nearby_devices: Arc::new(
                crate::features::discovery::nearby_devices::Devices::default(),
            ),
            ble_scans: Arc::new(crate::features::discovery::nearby_scan::Scans::default()),
            ble_scan: Arc::new(crate::features::services::discovery::ble_scan::Runtime::default()),
            pairing: Arc::new(crate::features::peers::pairing::sessions::Sessions::default()),
            dlna,
            archives: Arc::new(crate::features::services::files::archives::Archives::new(
                cache.clone(),
            )),
            upload_chunks: Arc::new(
                crate::features::services::files::upload_chunks::UploadChunks::new(
                    directory.join("uploads"),
                ),
            ),
            files: files,
            audio: audio,
            host,
            db,
            prefs,
            sms_sends: sms_sends.clone(),
            image_models: image_models.clone(),
            cast: Arc::new(
                crate::features::services::casting::dlna_sender_runtime::Runtime::default(),
            ),
            directory: directory.clone(),
            http_server: Arc::new(crate::features::services::http_status::Runtime::default()),
            web_root: Arc::new(std::sync::RwLock::new(None)),
            tls_identity: Arc::new(std::sync::RwLock::new(
                crate::infrastructure::tls_identity::IdentitySource::Json(
                    crate::infrastructure::data_paths::DataPaths::new(&directory).tls_identity(),
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
            thumbnails: Arc::new(thumbnails),
        };
        state
            .mms
            .set_resources(state.resources.clone(), receiver.clone());
        *state.buffered_writes_task.lock() =
            Some(crate::features::services::buffered_writes::start(
                state.file_platform.recent_files.clone(),
                state.duration_cache.clone(),
                state.db.clone(),
                receiver.clone(),
            ));
        Thumbnails::start(state.clone());
        crate::features::services::feeds::sync_service::schedule::start(state.clone());
        crate::features::services::chat::download_queue::start(state.clone());
        crate::features::services::peers::shared_batch::start(state.clone());
        crate::features::services::peers::aware::start(state.clone());
        *state.external_index_task.lock() = Some(
            crate::features::services::files::external_index::start(state.clone()),
        );
        state
            .management
            .start(state.clone())
            .map_err(|error| error.to_string())?;
        crate::features::services::discovery::pairing_timeout::start(
            state.pairing.clone(),
            state.events.clone(),
            receiver.clone(),
        );
        crate::features::services::discovery::nearby_devices::start(state.clone());
        crate::features::services::discovery::ble_scan::start(state.clone());
        *state.pomodoro_task.lock() = Some(state.pomodoro.follow(receiver.clone()));
        image_models.follow(receiver.clone());
        state.sms_sends.follow(receiver.clone());
        state.mms.follow(receiver.clone());
        crate::features::services::casting::receiver::control::follow_changes(&state);
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
        crate::features::services::sessions::native(self)
            .map(|session| session.url_token)
            .map_err(|error| error.to_string())
    }

    pub fn native_client_id(&self) -> Result<String, String> {
        if *self.stop.borrow() || self.stop.has_changed().is_err() {
            return Err("Rust core stopped".into());
        }
        self.prefs
            .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)
            .map_err(|error| error.to_string())?
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "native client identity unavailable".into())
    }

    pub async fn public_ports(&self) -> Option<(u16, u16)> {
        let state = self.http_server.snapshot();
        (state.http_port != 0).then_some((state.http_port as u16, state.https_port as u16))
    }
    pub async fn shutdown(&self, stop: &watch::Sender<bool>) {
        self.cast.stop_tasks();
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
                .call("peerTransportSocketCloseAll", plain_libs::serde_json::json!({}))
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
        self.thumbnails.shutdown().await;
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
        let writes = self.buffered_writes_task.lock().take();
        if let Some(task) = writes {
            task.await.expect("Buffered writes shutdown failed");
        }
        crate::features::services::buffered_writes::flush(
            self.file_platform.recent_files.clone(),
            self.duration_cache.clone(),
            self.db.clone(),
            true,
        )
        .await;
    }
}
