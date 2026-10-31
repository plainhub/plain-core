use axum::http::HeaderMap;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::net::{Ipv4Addr, TcpListener};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use subtle::ConstantTimeEq;
use tokio::{sync::watch, task::JoinHandle};
#[derive(Clone)]
pub(crate) struct ServerState {
    pub(crate) core: plain_rs::runtime::Runtime,
    pub(crate) peer_status_capacity: Arc<tokio::sync::Semaphore>,
    #[cfg(feature = "http_transport")]
    pub(crate) public_server: Arc<tokio::sync::Mutex<Option<PublicServer>>>,
    pub(crate) build_debug: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) web_bundle: Arc<std::sync::RwLock<Option<crate::http::web_bundle::SharedWebBundle>>>,
    pub(crate) public: crate::graphql::schemas::public::PublicSchema,
    pub(crate) guest_replay: Arc<crate::http::auth::replay::Replay>,
    pub(crate) main_replay: Arc<crate::http::auth::replay::Replay>,
    pub(crate) peer_schema: crate::graphql::schemas::peer::PeerSchema,
    pub(crate) subscription_sessions: Arc<crate::graphql::subscriptions::session::Sessions>,
    pub(crate) main_ws: Arc<crate::http::websocket::runtime::Runtime>,
    pub(crate) login_attempts: Arc<std::sync::Mutex<crate::http::auth::ws_login::LoginAttempts>>,
    pub(crate) shutdown: Arc<tokio::sync::Mutex<()>>,
}
impl std::ops::Deref for ServerState {
    type Target = plain_rs::runtime::Runtime;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}
impl ServerState {
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
    ) -> std::sync::Arc<plain_rs::services::casting::receiver::receiver_engine::DlnaEngine> {
        self.state.dlna.clone()
    }

    pub async fn close_native_scope(
        &self,
        scope: &plain_rs::host::request_scope::NativeRequestScope,
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
        plain_rs::services::files::file_access::resolve_native(&self.state, reference).await
    }

    pub async fn read_resource(
        &self,
        path: &str,
    ) -> Result<plain_rs::host::resources::NativeResourceReader, String> {
        let path = plain_rs::services::files::references::resolve(
            self.state.db.clone(),
            self.state.directory.clone(),
            path,
        )
        .map_err(|error| error.to_string())?;
        self.state
            .resources
            .read_native(&path, self.state.stop.clone())
            .await
            .map_err(|error| error.to_string())
    }

    pub fn peer_proxy_resolver(&self) -> crate::http::proxy::local::PeerResolver {
        let state = self.state.clone();
        Arc::new(move |id| {
            if state.native_client_id().is_err() {
                return None;
            }
            match plain_rs::services::peers::peer_address::proxy_address(&state.db, id) {
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
    ) -> Result<plain_rs::services::files::catalog_scan::NativeMediaCatalog, String> {
        self.state.native_client_id()?;
        Ok(
            plain_rs::services::files::catalog_scan::NativeMediaCatalog::new(
                self.state.core.clone(),
            ),
        )
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
        config: plain_rs::host::startup::StartupConfig,
    ) -> Result<(ServerState, watch::Sender<bool>, watch::Receiver<bool>), String> {
        let host = Arc::new(plain_rs::host::bridge::Host::with_startup_config(config)?);
        let subscription_sessions: Arc<crate::graphql::subscriptions::session::Sessions> =
            Arc::default();
        let main_ws = Arc::new(crate::http::websocket::runtime::Runtime::default());
        let login_attempts = Arc::new(std::sync::Mutex::new(
            crate::http::auth::ws_login::LoginAttempts::default(),
        ));
        let clients = Arc::new(crate::http::client_connections::ClientConnections {
            host: host.clone(),
            subscriptions: subscription_sessions.clone(),
            sockets: main_ws.clone(),
            login_attempts: login_attempts.clone(),
        });
        #[cfg(feature = "media")]
        let decoder = Some(Arc::new(plain_media::previews::images::ResourceDecoder)
            as Arc<dyn plain_rs::media::images::decoding::ThumbnailDecoder>);
        #[cfg(not(feature = "media"))]
        let decoder = None;
        let (core, stop) =
            plain_rs::runtime::Runtime::open(directory, token, prefs, host, clients, decoder)?;
        #[cfg(feature = "media")]
        plain_media::storage::paths::set(core.directory.clone(), core.cache.root().to_owned());
        let public = crate::graphql::schemas::public::build_with_runtime(core.clone());
        let receiver = core.stop.clone();
        let build_debug = core.host.startup().debug;
        let state = ServerState {
            core,
            public,
            peer_status_capacity: Arc::new(tokio::sync::Semaphore::new(128)),
            #[cfg(feature = "http_transport")]
            public_server: Arc::new(tokio::sync::Mutex::new(None)),
            peer_schema: crate::graphql::schemas::peer::schema(),
            guest_replay: Arc::new(crate::http::auth::replay::Replay::default()),
            main_replay: Arc::new(crate::http::auth::replay::Replay::default()),
            subscription_sessions,
            main_ws,
            login_attempts,
            build_debug: Arc::new(std::sync::atomic::AtomicBool::new(build_debug)),
            web_bundle: Arc::new(std::sync::RwLock::new(Some(Arc::new(
                crate::http::web_bundle::EmbeddedBundle,
            )))),
            shutdown: Arc::new(tokio::sync::Mutex::new(())),
        };
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
            plain_rs::data_paths::directory::DataDirectory::open(directory, cache_directory)?;
        let prefs = Arc::new(
            plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(directory))
                .map_err(|error| error.to_string())?,
        );
        let url_token = plain_rs::prefs::identity::ensure_url_token(&prefs);
        if plain_rs::utils::base64::base64_decode(&url_token).len() != 32 {
            return Err("token must contain 32 random bytes".into());
        }
        if prefs
            .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
            .map_err(|error| error.to_string())?
            .as_deref()
            != Some(url_token.as_str())
        {
            return Err("URL token persistence failed".into());
        }
        let token = plain_rs::crypto::gen_token();
        let mut config = plain_rs::platform::startup::config();
        config.cache_dir = Some(cache_directory.to_string_lossy().into_owned());
        Self::start_core_with_config(directory, &token, prefs, config)
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn start_core(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_core_with_config(
            directory,
            token,
            prefs,
            plain_rs::platform::startup::config(),
        )
    }

    pub fn start_core_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        config: plain_rs::host::startup::StartupConfig,
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
        Self::start_with_config(
            directory,
            token,
            prefs,
            plain_rs::platform::startup::config(),
        )
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(crate) fn start_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<plain_rs::prefs::Prefs>,
        config: plain_rs::host::startup::StartupConfig,
    ) -> Result<Self, String> {
        let (state, stop, mut receiver) =
            Self::build_state_with_config(directory, token, prefs, config)?;
        let router = crate::http::router::build_router(state.clone(), receiver.clone());
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
        config: plain_rs::host::startup::StartupConfig,
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
        crate::http::listeners::lifecycle::stop_locked(&self.state, &mut server).await;
        self.state.http_server.stopped();
        self.state.publish_http_status();
    }
    pub fn http_status(&self) -> plain_rs::services::http_status::HttpStatus {
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
    pub fn set_web_bundle(&self, bundle: crate::http::web_bundle::SharedWebBundle) {
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
        self.state.core.shutdown(&self.stop).await;
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
#[path = "../tests/unit/server.rs"]
mod tests;

#[cfg(feature = "http_transport")]
pub(crate) struct PublicServer {
    pub(crate) tls_certificate_sha256: String,
    pub(crate) discovery: crate::http::listeners::lifecycle::PublicDiscovery,
    pub(crate) listeners: crate::http::listeners::HttpListeners,
    pub(crate) stop: watch::Sender<bool>,
}
