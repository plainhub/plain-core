use std::{path::Path, sync::Arc};
#[path = "../../support/filesystem.rs"]
mod filesystem;
pub use filesystem::public_directory;
pub struct Core {
    state: crate::runtime::Runtime,
    stop: tokio::sync::watch::Sender<bool>,
}
struct Clients;
impl crate::infrastructure::host::client_connections::ClientConnections for Clients {
    fn close(&self, _: Option<&str>) {}
    fn revoke(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn contains(&self, _: &str) -> bool {
        false
    }
}
impl Core {
    pub fn start_core(
        directory: &Path,
        token: &str,
        prefs: Arc<crate::features::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_core_with_config(
            directory,
            token,
            prefs,
            crate::infrastructure::platform::startup::config(),
        )
    }
    pub fn start(
        directory: &Path,
        token: &str,
        prefs: Arc<crate::features::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_core(directory, token, prefs)
    }
    pub fn start_core_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<crate::features::prefs::Prefs>,
        config: crate::infrastructure::host::startup::StartupConfig,
    ) -> Result<Self, String> {
        let host =
            Arc::new(crate::infrastructure::host::bridge::Host::with_startup_config(config)?);
        let (state, stop) =
            crate::runtime::Runtime::open(directory, token, prefs, host, Arc::new(Clients))?;
        #[cfg(feature = "runtime-media")]
        state
            .media_scans
            .set_defaults(crate::features::files::index_scope::Defaults {
                excluded_directories: Vec::new(),
                excluded_names: Vec::new(),
            })?;
        Ok(Self { state, stop })
    }
    pub fn open(directory: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let prefs = Arc::new(
            crate::features::prefs::Prefs::load(&crate::features::prefs::default_path(directory))
                .map_err(|error| error.to_string())?,
        );
        crate::features::prefs::identity::ensure_url_token(&prefs);
        Self::start_core(directory, &plain_libs::crypto::gen_token(), prefs)
    }
    pub fn open_with_cache(directory: &Path, cache: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let prefs = Arc::new(
            crate::features::prefs::Prefs::load(&crate::features::prefs::default_path(directory))
                .map_err(|error| error.to_string())?,
        );
        crate::features::prefs::identity::ensure_url_token(&prefs);
        let mut config = crate::infrastructure::platform::startup::config();
        config.cache_dir = Some(cache.to_string_lossy().into_owned());
        Self::start_core_with_config(directory, &plain_libs::crypto::gen_token(), prefs, config)
    }
    pub fn runtime_state(&self) -> crate::runtime::Runtime {
        self.state.clone()
    }
    pub async fn stop_core(&self) {
        self.state.shutdown(&self.stop).await;
    }
    pub async fn shutdown(self) {
        self.stop_core().await;
    }
}
impl Drop for Core {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
