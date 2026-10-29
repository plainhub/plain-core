use axum::http::HeaderMap;
use plain_rs::db::Db;
use plain_rs::events::WsEvent;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::net::{Ipv4Addr, TcpListener};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use subtle::ConstantTimeEq;
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
};
#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) ble_advertising: Arc<crate::services::discovery::ble_advertising::Runtime>,
    _data_directory: Arc<crate::assembly::data_directory::DataDirectory>,
    pub(crate) cache: plain_rs::cache_paths::CachePaths,
    pub(crate) cache_changes: Arc<tokio::sync::RwLock<()>>,
    pub(crate) remote_cache: Arc<crate::services::files::remote_cache::RemoteCache>,
    pub(crate) logs: Arc<crate::services::logs::Logs>,
    pub(crate) device: Arc<crate::services::device::Status>,
    pub(crate) browser: Arc<crate::services::files::browser::Runtime>,
    pub(crate) file_platform: Arc<crate::services::files::platform::FilePlatform>,
    pub(crate) feeds: Arc<crate::services::feeds::SyncService>,
    pomodoro_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    external_index_task: Arc<parking_lot::Mutex<Option<JoinHandle<()>>>>,
    pub(crate) update_checks: Arc<crate::services::settings::updates::Runtime>,
    pub(crate) duration_updates: Arc<crate::services::files::duration_updates::Runtime>,
    #[cfg(test)]
    pub(crate) duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    pub(crate) store: Arc<plain_rs::kv::LazyDb>,
    #[cfg(feature = "http_transport")]
    pub(crate) public_server: Arc<tokio::sync::Mutex<Option<PublicServer>>>,
    pub(crate) http_server: Arc<crate::services::http_server::Runtime>,
    pub(crate) build_debug: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) web_root: Arc<std::sync::RwLock<Option<PathBuf>>>,
    pub(crate) web_bundle:
        Arc<std::sync::RwLock<Option<crate::http::routes::web_bundle::SharedWebBundle>>>,
    pub(crate) tls_identity: Arc<std::sync::RwLock<crate::services::tls_identity::IdentitySource>>,
    pub(crate) public: crate::graphql::schemas::public::PublicSchema,
    pub(crate) aware: Arc<crate::services::peers::aware::Runtime>,
    pub(crate) peer_status: Arc<crate::services::peers::peer_status::Runtime>,
    pub(crate) management: Arc<crate::services::peers::management::Runtime>,
    pub(crate) clipboard_changes: Arc<crate::services::clipboard::Changes>,
    pub(crate) network_revision: watch::Sender<u64>,
    pub(crate) mdns: Arc<crate::services::discovery::mdns_runtime::Runtime>,
    pub(crate) guest_replay: Arc<crate::http::auth::replay::Replay>,
    pub(crate) main_replay: Arc<crate::http::auth::replay::Replay>,
    pub(crate) peer_schema: crate::graphql::schemas::peer::PeerSchema,
    pub(crate) archives: Arc<crate::services::files::archives::Archives>,
    pub(crate) upload_chunks: Arc<crate::services::files::upload_chunks::UploadChunks>,
    pub(crate) files: Arc<crate::services::files::file_tasks::FileTasks>,
    #[cfg(feature = "media")]
    pub(crate) media_scans: Arc<crate::services::files::media_scans::MediaScans>,
    /// The live media watcher. Held so `stop_core` can release it; the
    /// catalog replaces it rather than stacking a second watch.
    #[cfg(feature = "media")]
    pub(crate) media_watch: std::sync::Arc<
        std::sync::Mutex<Option<crate::services::files::media_watch::WatcherHandle>>,
    >,
    pub(crate) host: Arc<crate::host::bridge::Host>,
    pub(crate) thumbnails: Arc<crate::services::images::thumbnails::Thumbnails>,
    pub(crate) db: Arc<Db>,
    pub(crate) lan: Arc<crate::services::peers::peer_lan::Lan>,
    pub(crate) transport: Arc<plain_rs::transport::router::Router>,
    pub(crate) transport_delivery: Arc<std::sync::Mutex<bool>>,
    pub(crate) previews: Arc<crate::services::link_preview::Schedule>,
    pub(crate) prewarmer: Arc<plain_rs::chat::attachments::prewarm::Prewarmer>,
    pub(crate) shared_batches: Arc<crate::services::peers::shared_batch::Runtime>,
    pub(crate) download_runtime: Arc<crate::services::chat::download_queue::Runtime>,
    pub(crate) downloads: Arc<plain_rs::chat::attachments::download_queue::Queue>,
    pub(crate) delivery: Arc<plain_rs::chat::delivery::Delivery>,
    pub(crate) channel_delivery: Arc<tokio::sync::Semaphore>,
    pub(crate) ble_transport: Arc<crate::services::discovery::ble_http::Runtime>,
    pub(crate) ble_pairing: Arc<crate::services::discovery::ble_pairing::Pairings>,
    pub(crate) pairing_runtime: Arc<crate::services::discovery::pairing_runtime::Runtime>,
    pub(crate) nearby_devices: Arc<plain_rs::discovery::nearby_devices::Devices>,
    pub(crate) ble_scans: Arc<plain_rs::discovery::nearby_scan::Scans>,
    pub(crate) ble_scan: Arc<crate::services::discovery::ble_scan::Runtime>,
    pub(crate) pairing: Arc<plain_rs::peers::pairing::sessions::Sessions>,
    pub(crate) prefs: Arc<plain_rs::prefs::Prefs>,
    pub(crate) dlna: Arc<crate::services::casting::receiver::receiver_engine::DlnaEngine>,
    pub(crate) sms_sends: Arc<crate::services::sms::send_runtime::Runtime>,
    pub(crate) image_models: Arc<crate::services::images::models::Runtime>,
    #[cfg(test)]
    pub(crate) image_index: Arc<crate::services::images::index::ImageIndex>,
    pub(crate) mobile: Arc<crate::mobile::Runtime>,
    pub(crate) cast: Arc<crate::services::casting::dlna_sender_runtime::Runtime>,
    pub(crate) audio: Arc<crate::services::audio::host::Audio>,
    pub(crate) subscription_sessions: Arc<crate::graphql::subscriptions::session::Sessions>,
    pub(crate) main_ws: Arc<crate::http::websocket::host::Runtime>,
    pub(crate) login_attempts: Arc<std::sync::Mutex<crate::http::auth::ws_login::LoginAttempts>>,
    pub(crate) directory: std::path::PathBuf,
    pub(crate) token: Arc<str>,
    pub(crate) temp_values: crate::services::temp_values::TempValues,
    pub(crate) events: broadcast::Sender<WsEvent>,
    pub(crate) shutdown: Arc<tokio::sync::Mutex<()>>,
    pub(crate) stop: watch::Receiver<bool>,
    pub(crate) bridge: Arc<crate::host::resources::NativeResources>,
}
impl ServerState {
    pub(crate) fn native_url_token(&self) -> Result<String, String> {
        self.native_client_id()?;
        self.prefs
            .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
            .map_err(|error| error.to_string())?
            .filter(|token| !token.is_empty())
            .ok_or_else(|| "native URL token unavailable".into())
    }

    pub(crate) fn native_client_id(&self) -> Result<String, String> {
        if *self.stop.borrow() || self.stop.has_changed().is_err() {
            return Err("Rust core stopped".into());
        }
        self.prefs
            .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)
            .map_err(|error| error.to_string())?
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "native client identity unavailable".into())
    }

    pub(crate) async fn public_ports(&self) -> Option<(u16, u16)> {
        #[cfg(feature = "http_transport")]
        {
            self.public_server
                .lock()
                .await
                .as_ref()
                .map(|server| (server.listeners.http_port, server.listeners.https_port))
        }
        #[cfg(not(feature = "http_transport"))]
        {
            None
        }
    }

    pub(crate) fn authenticated(&self, headers: &HeaderMap) -> bool {
        let candidate = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or_default();
        candidate.as_bytes().ct_eq(self.token.as_bytes()).into()
    }
}
pub struct ContentServer {
    pub port: u16,
    state: ServerState,
    task: Option<JoinHandle<()>>,
    stop: watch::Sender<bool>,
}
impl ContentServer {
    #[cfg(feature = "headless")]
    pub(crate) fn headless_state(&self) -> &ServerState {
        &self.state
    }

    pub fn shared_preferences(&self) -> Result<Arc<plain_rs::prefs::Prefs>, String> {
        self.state.native_client_id()?;
        Ok(self.state.prefs.clone())
    }

    pub fn preferences(&self) -> Result<&plain_rs::prefs::Prefs, String> {
        self.state.native_client_id()?;
        Ok(&self.state.prefs)
    }

    pub fn dlna_engine(
        &self,
    ) -> std::sync::Arc<crate::services::casting::receiver::receiver_engine::DlnaEngine> {
        self.state.dlna.clone()
    }

    pub async fn close_native_scope(
        &self,
        scope: &crate::native::graphql::NativeRequestScope,
    ) -> Result<(), String> {
        scope.close();
        self.state
            .mdns
            .release_native_scope(scope)
            .await
            .map_err(|error| error.to_string())
    }

    pub fn native_management(&self) -> Result<crate::native::management::NativeManagement, String> {
        self.state.native_client_id()?;
        Ok(crate::native::management::NativeManagement::new(
            self.state.clone(),
        ))
    }

    pub fn native_graphql(&self) -> Result<crate::native::graphql::NativeGraphqlExecutor, String> {
        let client_id = self.state.native_client_id()?;
        Ok(crate::native::graphql::NativeGraphqlExecutor::new(
            self.state.clone(),
            client_id,
        ))
    }

    pub fn native_uploads(&self) -> Result<crate::native::uploads::NativeUploads, String> {
        self.state.native_client_id()?;
        Ok(crate::native::uploads::NativeUploads::new(
            self.state.upload_chunks.clone(),
            self.state.stop.clone(),
        ))
    }

    pub fn graphql_executor(&self) -> crate::graphql::executor::GraphqlExecutor {
        crate::graphql::executor::GraphqlExecutor::new(self.state.clone())
    }
    pub async fn resolve_native_file(&self, reference: &str) -> Result<PathBuf, String> {
        crate::services::files::file_access::resolve_native(&self.state, reference).await
    }

    pub async fn read_resource(
        &self,
        path: &str,
    ) -> Result<crate::host::resources::NativeResourceReader, String> {
        let path = crate::host::filesystem::resolve(&self.state, path)
            .await
            .map_err(|error| error.to_string())?;
        self.state
            .bridge
            .read_native(&path, self.state.stop.clone())
            .await
    }

    pub fn peer_proxy_resolver(&self) -> crate::http::proxy::local::PeerResolver {
        let state = self.state.clone();
        Arc::new(move |id| {
            if state.native_client_id().is_err() {
                return None;
            }
            match crate::services::peers::peer_address::proxy_address(&state.db, id) {
                Ok(address) => address.map(|address| {
                    let state = state.clone();
                    crate::http::proxy::local::ResolvedPeer::new(address, move || {
                        if state.native_client_id().is_ok() {
                            state.mdns.browse_resident();
                        }
                    })
                }),
                Err(error) => {
                    log::warn!("Peer proxy address lookup failed: {error}");
                    None
                }
            }
        })
    }

    #[cfg(feature = "media")]
    pub fn native_audio(&self) -> Result<crate::native::audio::NativeAudio, String> {
        self.state.native_client_id()?;
        Ok(crate::native::audio::NativeAudio::new(self.state.clone()))
    }

    pub fn native_media_catalog(
        &self,
    ) -> Result<crate::native::media_catalog::NativeMediaCatalog, String> {
        self.state.native_client_id()?;
        Ok(crate::native::media_catalog::NativeMediaCatalog::new(
            self.state.clone(),
        ))
    }

    pub fn log_file(&self) -> Result<PathBuf, String> {
        self.state.logs.path()
    }

    pub fn set_log_file(&self, path: &Path) -> Result<(), String> {
        self.state.logs.set_path(path.to_owned())
    }

    pub fn data_directory(&self) -> &Path {
        &self.state.directory
    }

    pub fn host_events(&self) -> Result<crate::host::events::HostEvents, String> {
        crate::host::events::HostEvents::new(self.state.clone())
    }

    fn build_state_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        config: crate::host::startup::StartupConfig,
    ) -> Result<(ServerState, watch::Sender<bool>, watch::Receiver<bool>), String> {
        crate::assembly::core_dependencies::CoreDependencies::validate_token(token)?;
        if !directory.is_absolute() {
            return Err("Absolute core data directory required".into());
        }
        let host = Arc::new(crate::host::bridge::Host::with_startup_config(config)?);
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let cache_directory = match host.startup().cache_dir.as_deref() {
            Some(path) => PathBuf::from(path),
            None => plain_rs::cache_paths::system::resolve(directory)?,
        };
        let data_directory =
            crate::assembly::data_directory::DataDirectory::open(directory, &cache_directory)?;
        let paths = plain_rs::data_paths::DataPaths::new(directory);
        let db = Arc::new(Db::open(&paths.database()).map_err(|e| e.to_string())?);
        let (events, _) = broadcast::channel(256);
        Self::build_state_with(
            crate::assembly::core_dependencies::CoreDependencies::new(
                db,
                prefs,
                directory.to_path_buf(),
                token.to_owned(),
                events,
                host,
            ),
            data_directory,
        )
    }

    fn build_state_with(
        dependencies: crate::assembly::core_dependencies::CoreDependencies,
        data_directory: Arc<crate::assembly::data_directory::DataDirectory>,
    ) -> Result<(ServerState, watch::Sender<bool>, watch::Receiver<bool>), String> {
        let crate::assembly::core_dependencies::CoreDependencies {
            db,
            prefs,
            directory,
            token,
            events,
            host,
        } = dependencies;
        #[cfg(feature = "media")]
        plain_media::storage::paths::set(
            directory.clone(),
            data_directory.cache_paths().root().to_owned(),
        );
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
            Arc::new(crate::services::peers::peer_lan::Lan::new().map_err(|e| e.to_string())?);
        let services = crate::assembly::services::Services::new(
            db.clone(),
            host.clone(),
            events.clone(),
            prefs.clone(),
            directory.clone(),
            data_directory.cache_paths().clone(),
        );
        services.device.fixed_info()?;
        let mobile = crate::mobile::Runtime::new(
            host.clone(),
            db.clone(),
            directory.clone(),
            services.store.clone(),
            events.clone(),
        );
        let image_models = crate::services::images::models::Runtime::new(
            directory.clone(),
            host.clone(),
            prefs.clone(),
            services.store.clone(),
            services.index.clone(),
            events.clone(),
        );
        let store = services.store.clone();
        let public = crate::graphql::schemas::public::build_with_runtime(
            host.clone(),
            events.clone(),
            prefs.clone(),
            db.clone(),
            services.clone(),
            image_models.clone(),
            mobile.clone(),
        );
        let bridge = services.resources.clone();
        let attachments =
            Arc::new(plain_rs::chat::attachments::attachment_imports::Imports::default());
        let downloads = Arc::new(plain_rs::chat::attachments::download_queue::Queue::new(
            (*db).clone(),
            directory.clone(),
            attachments.clone(),
        ));
        let preview_events = events.clone();
        let previews = Arc::new(crate::services::link_preview::Schedule::new(
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
        let dlna = Arc::new(crate::services::casting::receiver::receiver_engine::DlnaEngine::new());
        let build_debug = host.startup().debug;
        let state = ServerState {
            ble_advertising: Arc::new(
                crate::services::discovery::ble_advertising::Runtime::default(),
            ),
            cache: data_directory.cache_paths().clone(),
            _data_directory: data_directory,
            logs: services.logs.clone(),
            device: services.device.clone(),
            file_platform: services.file_platform.clone(),
            browser: services.browser.clone(),
            cache_changes: Arc::new(tokio::sync::RwLock::new(())),
            remote_cache: services.remote_cache.clone(),
            feeds: services.feeds.clone(),
            pomodoro_task: Arc::new(parking_lot::Mutex::new(None)),
            external_index_task: Default::default(),
            update_checks: Arc::new(crate::services::settings::updates::Runtime::default()),
            #[cfg(test)]
            duration_cache: services.duration_cache.clone(),
            duration_updates: services.duration_updates.clone(),
            store,
            #[cfg(feature = "media")]
            media_scans: services.media_scans.clone(),
            #[cfg(feature = "media")]
            media_watch: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(feature = "http_transport")]
            public_server: Arc::new(tokio::sync::Mutex::new(None)),
            public,
            lan,
            thumbnails: Arc::new(crate::services::images::thumbnails::Thumbnails::default()),
            aware: Arc::new(crate::services::peers::aware::Runtime::default()),
            peer_status: Arc::new(crate::services::peers::peer_status::Runtime::default()),
            management: Arc::new(crate::services::peers::management::Runtime::default()),
            clipboard_changes: Arc::new(crate::services::clipboard::Changes::default()),
            network_revision: watch::channel(0).0,
            mdns: Arc::new(crate::services::discovery::mdns_runtime::Runtime::default()),
            peer_schema: crate::graphql::schemas::peer::schema(),
            guest_replay: Arc::new(crate::http::auth::replay::Replay::default()),
            main_replay: Arc::new(crate::http::auth::replay::Replay::default()),
            transport: Arc::new(plain_rs::transport::router::Router::default()),
            transport_delivery: Arc::new(std::sync::Mutex::new(false)),
            previews,
            prewarmer: Arc::new(plain_rs::chat::attachments::prewarm::Prewarmer::default()),
            shared_batches: Arc::new(crate::services::peers::shared_batch::Runtime::default()),
            download_runtime: Arc::new(crate::services::chat::download_queue::Runtime::default()),
            downloads,
            delivery: Arc::new(plain_rs::chat::delivery::Delivery::new((*db).clone())),
            channel_delivery: Arc::new(tokio::sync::Semaphore::new(4)),
            ble_transport: Arc::new(crate::services::discovery::ble_http::Runtime::default()),
            ble_pairing: Arc::new(crate::services::discovery::ble_pairing::Pairings::default()),
            pairing_runtime: Arc::new(
                crate::services::discovery::pairing_runtime::Runtime::default(),
            ),
            nearby_devices: Arc::new(plain_rs::discovery::nearby_devices::Devices::default()),
            ble_scans: Arc::new(plain_rs::discovery::nearby_scan::Scans::default()),
            ble_scan: Arc::new(crate::services::discovery::ble_scan::Runtime::default()),
            pairing: Arc::new(plain_rs::peers::pairing::sessions::Sessions::default()),
            dlna,
            archives: services.archives,
            upload_chunks: services.upload_chunks,
            files: services.files,
            #[cfg(test)]
            image_index: services.index.clone(),
            audio: services.audio,
            host,
            db,
            prefs,
            sms_sends: services.sms_sends.clone(),
            image_models: image_models.clone(),
            mobile,
            cast: Arc::new(crate::services::casting::dlna_sender_runtime::Runtime::default()),
            subscription_sessions: Arc::default(),
            main_ws: Arc::new(crate::http::websocket::host::Runtime::default()),
            login_attempts: Arc::new(std::sync::Mutex::new(
                crate::http::auth::ws_login::LoginAttempts::default(),
            )),
            directory: directory.clone(),
            http_server: Arc::new(crate::services::http_server::Runtime::default()),
            build_debug: Arc::new(std::sync::atomic::AtomicBool::new(build_debug)),
            web_root: Arc::new(std::sync::RwLock::new(None)),
            web_bundle: Arc::new(std::sync::RwLock::new(Some(Arc::new(
                crate::http::routes::web_bundle::EmbeddedBundle,
            )))),
            tls_identity: Arc::new(std::sync::RwLock::new(
                crate::services::tls_identity::IdentitySource::Json(
                    plain_rs::data_paths::DataPaths::new(&directory).tls_identity(),
                ),
            )),
            token: Arc::from(token.as_str()),
            temp_values: services.temp_values.clone(),
            events,
            shutdown: Arc::new(tokio::sync::Mutex::new(())),
            stop: receiver.clone(),
            bridge: bridge.clone(),
        };
        state
            .mobile
            .mms
            .set_resources(bridge.clone(), receiver.clone());
        crate::services::feeds::sync_service::schedule::start(state.clone());
        crate::services::chat::download_queue::start(state.clone());
        crate::services::peers::shared_batch::start(state.clone());
        crate::services::peers::aware::start(state.clone());
        *state.external_index_task.lock() =
            Some(crate::services::files::external_index::start(state.clone()));
        state
            .management
            .start(state.clone())
            .map_err(|error| error.to_string())?;
        crate::services::discovery::pairing_timeout::start(
            state.pairing.clone(),
            state.events.clone(),
            receiver.clone(),
        );
        crate::services::discovery::nearby_devices::start(state.clone());
        crate::services::discovery::ble_scan::start(state.clone());
        *state.pomodoro_task.lock() = Some(services.pomodoro.follow(receiver.clone()));
        image_models.follow(receiver.clone());
        state.sms_sends.follow(receiver.clone());
        state.mobile.mms.follow(receiver.clone());
        crate::services::casting::receiver::control::follow_changes(&state);
        let mdns = state.mdns.clone();
        let mdns_host = state.host.clone();
        let mut mdns_stop = receiver.clone();
        tokio::spawn(async move {
            if !*mdns_stop.borrow() {
                let _ = mdns_stop.changed().await;
            }
            mdns.close(&mdns_host).await;
        });
        Ok((state, stop, receiver))
    }

    pub fn cache_paths(&self) -> &plain_rs::cache_paths::CachePaths {
        &self.state.cache
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn open(directory: &Path) -> Result<Self, String> {
        let cache = plain_rs::cache_paths::system::resolve(directory)?;
        Self::open_with_cache(directory, &cache)
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn open_with_cache(directory: &Path, cache_directory: &Path) -> Result<Self, String> {
        tokio::runtime::Handle::try_current().map_err(|error| error.to_string())?;
        if !directory.is_absolute() {
            return Err("absolute core data directory required".into());
        }
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let _data_directory =
            crate::assembly::data_directory::DataDirectory::open(directory, cache_directory)?;
        let prefs = Arc::new(
            plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(directory))
                .map_err(|error| error.to_string())?,
        );
        let url_token = plain_rs::prefs::identity::ensure_url_token(&prefs);
        crate::assembly::core_dependencies::CoreDependencies::validate_token(&url_token)?;
        if prefs
            .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
            .map_err(|error| error.to_string())?
            .as_deref()
            != Some(url_token.as_str())
        {
            return Err("URL token persistence failed".into());
        }
        let token = plain_rs::crypto::gen_token();
        let mut config = crate::platform::startup::config();
        config.cache_dir = Some(cache_directory.to_string_lossy().into_owned());
        Self::start_core_with_config(directory, &token, prefs, config)
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn start_core(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_core_with_config(directory, token, prefs, crate::platform::startup::config())
    }

    pub fn start_core_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        config: crate::host::startup::StartupConfig,
    ) -> Result<Self, String> {
        let (state, stop, _) = Self::build_state_with_config(directory, token, prefs, config)?;
        Ok(Self {
            port: 0,
            state,
            task: None,
            stop,
        })
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn start(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_with_config(directory, token, prefs, crate::platform::startup::config())
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(crate) fn start_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        config: crate::host::startup::StartupConfig,
    ) -> Result<Self, String> {
        let (state, stop, mut receiver) =
            Self::build_state_with_config(directory, token, prefs, config)?;
        let router = crate::http::routes::public::build_router(state.clone(), receiver.clone());
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let listener = tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;
        let task = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async move {
                let _ = receiver.changed().await;
            })
            .await;
        });
        Ok(Self {
            port,
            state,
            task: Some(task),
            stop,
        })
    }
    #[cfg(feature = "http_transport")]
    pub async fn start_http(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        config: crate::host::startup::StartupConfig,
        http: u16,
        https: u16,
        cert: Vec<u8>,
        key: Vec<u8>,
    ) -> Result<Self, String> {
        let mut server = Self::start_core_with_config(directory, token, prefs, config)?;
        server.port = server.start_public(http, https, cert, key).await?.0;
        Ok(server)
    }
    #[cfg(test)]
    pub(crate) fn runtime_state(&self) -> ServerState {
        self.state.clone()
    }
    #[cfg(feature = "http_transport")]
    pub async fn start_public(
        &self,
        http: u16,
        https: u16,
        cert: Vec<u8>,
        key: Vec<u8>,
    ) -> Result<(u16, u16), String> {
        let _core = self.state.shutdown.lock().await;
        self.state.start_public_locked(http, https, cert, key).await
    }

    pub fn native_url_token(&self) -> Result<String, String> {
        self.state.native_url_token()
    }

    #[cfg(feature = "http_transport")]
    pub async fn start_configured_public(&self) -> Result<(u16, u16), String> {
        let _core = self.state.shutdown.lock().await;
        self.state.native_client_id()?;
        let (http, https, cert, key) = match self.state.http_configuration().await {
            Ok(configuration) => configuration,
            Err(error) => {
                if self.state.public_server.lock().await.is_none() {
                    self.state.http_server.failed(error.clone());
                    self.state.publish_http_status();
                }
                return Err(error);
            }
        };
        self.state.start_public_locked(http, https, cert, key).await
    }

    #[cfg(feature = "http_transport")]
    pub async fn stop_public(&self) {
        let _core = self.state.shutdown.lock().await;
        self.stop_public_locked().await;
    }

    #[cfg(feature = "http_transport")]
    async fn stop_public_locked(&self) {
        let mut server = self.state.public_server.lock().await.take();
        crate::assembly::lifecycle::stop_locked(&self.state, &mut server).await;
        self.state.http_server.stopped();
        self.state.publish_http_status();
    }
    pub fn http_status(&self) -> crate::services::http_server::HttpStatus {
        self.state.http_server.status()
    }
    #[cfg(feature = "http_transport")]
    pub async fn http_ports(&self) -> Option<(u16, u16)> {
        self.state.public_ports().await
    }
    pub fn set_build_debug(&self, debug: bool) {
        self.state
            .build_debug
            .store(debug, std::sync::atomic::Ordering::Relaxed);
    }
    pub fn set_web_root(&self, root: &str) {
        if let Ok(mut guard) = self.state.web_root.write() {
            *guard = (!root.is_empty()).then(|| PathBuf::from(root));
        }
    }
    /// The bundle compiled into this executable, used only when no web root
    /// is set.
    pub fn set_web_bundle(&self, bundle: crate::http::routes::web_bundle::SharedWebBundle) {
        if let Ok(mut guard) = self.state.web_bundle.write() {
            *guard = Some(bundle);
        }
    }
    pub async fn stop_core(&self) {
        let _shutdown = self.state.shutdown.lock().await;
        self.state.subscription_sessions.close_client(None);
        if *self.state.stop.borrow() {
            return;
        }
        #[cfg(feature = "http_transport")]
        self.stop_public_locked().await;
        self.state
            .shared_batches
            .shutdown(&self.state.directory)
            .await;
        let external_index = self.state.external_index_task.lock().take();
        if let Some(task) = external_index {
            task.abort();
            let _ = task.await;
        }
        self.state.feeds.shutdown().await;
        self.state.download_runtime.shutdown().await;
        self.state.update_checks.download.shutdown().await;
        self.state.mobile.mms.shutdown().await;
        self.state.image_models.shutdown().await;
        self.state.duration_updates.shutdown().await;
        if self.state.host.connected() && self.state.host.needs_socket_cleanup() {
            let _ = self
                .state
                .host
                .call("peerTransportSocketCloseAll", serde_json::json!({}))
                .await;
        }
        #[cfg(feature = "media")]
        {
            // The watcher writes into the catalog, so it has to stop before
            // the scan state it feeds is closed.
            if let Ok(mut slot) = self.state.media_watch.lock() {
                slot.take();
            }
            if let Err(error) = self.state.media_scans.close() {
                log::error!("Media scan cancellation failed: {error}");
            }
        }
        let _ = self.stop.send(true);
        self.state.management.shutdown().await;
        let pomodoro_task = self.state.pomodoro_task.lock().take();
        if let Some(task) = pomodoro_task {
            if let Err(error) = task.await {
                log::warn!("Pomodoro shutdown: {error}");
            }
        }
        #[cfg(feature = "media")]
        if let Err(error) = self.state.media_scans.wait_background().await {
            log::error!("Media scan task shutdown failed: {error}");
        }
        #[cfg(feature = "http_transport")]
        self.state.mdns.close(&self.state.host).await;
    }

    pub async fn shutdown(mut self) {
        self.stop_core().await;
        if let Some(task) = &mut self.task {
            let _ = task.await;
        }
    }
}
impl Drop for ContentServer {
    fn drop(&mut self) {
        self.state.subscription_sessions.close_client(None);
        let _ = self.stop.send(true);
        if let Some(task) = &self.task {
            task.abort();
        }
        #[cfg(feature = "http_transport")]
        if let Ok(mut guard) = self.state.public_server.try_lock() {
            guard.take();
        }
    }
}
#[cfg(test)]
#[path = "../../tests/unit/assembly/server.rs"]
mod tests;

#[cfg(feature = "http_transport")]
pub(crate) struct PublicServer {
    pub(crate) tls_certificate_sha256: String,
    pub(crate) discovery: crate::assembly::lifecycle::PublicDiscovery,
    pub(crate) listeners: crate::http::listeners::HttpListeners,
    pub(crate) stop: watch::Sender<bool>,
}
