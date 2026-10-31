use std::{path::Path, sync::Arc};
pub struct Core {
    state: crate::runtime::Runtime,
    stop: tokio::sync::watch::Sender<bool>,
}
struct Clients;
impl crate::host::client_connections::ClientConnections for Clients {
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
        prefs: Arc<crate::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_core_with_config(directory, token, prefs, crate::platform::startup::config())
    }
    pub fn start(
        directory: &Path,
        token: &str,
        prefs: Arc<crate::prefs::Prefs>,
    ) -> Result<Self, String> {
        Self::start_core(directory, token, prefs)
    }
    pub fn start_core_with_config(
        directory: &Path,
        token: &str,
        prefs: Arc<crate::prefs::Prefs>,
        config: crate::host::startup::StartupConfig,
    ) -> Result<Self, String> {
        let host = Arc::new(crate::host::bridge::Host::with_startup_config(config)?);
        let (state, stop) =
            crate::runtime::Runtime::open(directory, token, prefs, host, Arc::new(Clients), None)?;
        Ok(Self { state, stop })
    }
    pub fn open(directory: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let prefs = Arc::new(
            crate::prefs::Prefs::load(&crate::prefs::default_path(directory))
                .map_err(|error| error.to_string())?,
        );
        crate::prefs::identity::ensure_url_token(&prefs);
        Self::start_core(directory, &crate::crypto::gen_token(), prefs)
    }
    pub fn open_with_cache(directory: &Path, cache: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let prefs = Arc::new(
            crate::prefs::Prefs::load(&crate::prefs::default_path(directory))
                .map_err(|error| error.to_string())?,
        );
        crate::prefs::identity::ensure_url_token(&prefs);
        let mut config = crate::platform::startup::config();
        config.cache_dir = Some(cache.to_string_lossy().into_owned());
        Self::start_core_with_config(directory, &crate::crypto::gen_token(), prefs, config)
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
