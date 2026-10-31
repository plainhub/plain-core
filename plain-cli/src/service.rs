use crate::host::Host;
use crate::login_socket as login;
use crate::storage;
use anyhow::{Result, anyhow};
use plain_rs::platform::system::consts::AppPaths;
use plain_server::server::ContentServer;
use std::{path::PathBuf, sync::Arc};

#[derive(Default)]
pub struct Options {
    pub media_directories: Vec<PathBuf>,
    pub web_root: Option<PathBuf>,
    pub automount: bool,
}

pub struct Service {
    core: Arc<ContentServer>,
    host: tokio::task::JoinHandle<Result<(), String>>,
    login: Option<login::Service>,
    #[cfg(target_os = "linux")]
    watcher: Option<plain_rs::platform::linux::mounts::automount::Watcher>,
}

impl Service {
    pub async fn start(paths: &AppPaths, options: Options) -> Result<Self> {
        storage::validate_automount(options.automount)?;
        crate::bootstrap::initialize_tls();
        let core = Arc::new(
            ContentServer::open_with_cache(&paths.data_dir, &paths.cache_dir)
                .map_err(anyhow::Error::msg)?,
        );
        let log_file = plain_rs::platform::system::log::default_log_file(&paths.data_dir);
        plain_rs::platform::system::log::set_file(&log_file)?;
        core.set_log_file(&log_file).map_err(anyhow::Error::msg)?;
        let runtime = Arc::new(Host::new(core.clone(), capabilities()));
        let host = tokio::spawn(runtime.run());
        let mut service = Self {
            core,
            host,
            login: None,
            #[cfg(target_os = "linux")]
            watcher: None,
        };
        if let Err(error) = service.start_services(paths, options).await {
            service.shutdown().await;
            return Err(error);
        }
        Ok(service)
    }

    async fn start_services(&mut self, paths: &AppPaths, options: Options) -> Result<()> {
        self.login = login::Service::start(&self.core).await?;
        if self.login.is_some() {
            log::info!(
                "Use plainapp login requests with the same data directory to approve Web access"
            );
        }
        #[cfg(target_os = "linux")]
        if options.automount {
            let store = self.core.runtime().store.get()?;
            let events = storage::event_sink(&self.core, options.media_directories.is_empty())
                .map_err(anyhow::Error::msg)?;
            let mount_store = store.clone();
            let mount_events = events.clone();
            tokio::task::spawn_blocking(move || {
                plain_rs::platform::linux::mounts::automount::ensure_mounted_usb_volumes(
                    &mount_store,
                    &mount_events,
                )
            })
            .await??;
            self.watcher =
                Some(plain_rs::platform::linux::mounts::automount::Watcher::start(store, events));
        }
        let prefs = self.core.shared_preferences().map_err(anyhow::Error::msg)?;
        let roots = storage::source_directories(&prefs, &options.media_directories)?;
        let catalog = self
            .core
            .native_media_catalog()
            .map_err(anyhow::Error::msg)?;
        catalog
            .set_source_directories(&roots)
            .map_err(anyhow::Error::msg)?;
        catalog
            .configure_scan_directories(None)
            .map_err(anyhow::Error::msg)?;
        catalog
            .watch_source_directories()
            .map_err(anyhow::Error::msg)?;
        *self
            .core
            .runtime()
            .tls_identity
            .write()
            .map_err(|_| anyhow!("TLS identity configuration unavailable"))? =
            plain_rs::services::tls_identity::IdentitySource::Pem {
                certificate: paths.tls_cert.clone(),
                key: paths.tls_key.clone(),
            };
        if let Some(root) = options.web_root {
            anyhow::ensure!(
                root.is_dir(),
                "Web directory does not exist: {}",
                root.display()
            );
            self.core.set_web_root(&root.to_string_lossy());
        }
        let ports = self
            .core
            .start_configured_public()
            .await
            .map_err(anyhow::Error::msg)?;
        log::info!("HTTP server listening on :{}", ports.0);
        log::info!("HTTPS server listening on :{}", ports.1);
        if !roots.is_empty() {
            storage::start_source_scan(&catalog).map_err(anyhow::Error::msg)?;
        }
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(watcher) = &mut self.watcher {
            return tokio::select! {
                result = login::wait(&mut self.login) => result,
                result = &mut self.host => {
                    result.map_err(anyhow::Error::from)?.map_err(anyhow::Error::msg)?;
                    Err(anyhow!("Headless host session stopped"))
                }
                result = watcher.wait() => {
                    result?;
                    Err(anyhow!("Mount watcher stopped"))
                }
            };
        }
        tokio::select! {
            result = login::wait(&mut self.login) => result,
            result = &mut self.host => {
                result?.map_err(anyhow::Error::msg)?;
                Err(anyhow!("Headless host session stopped"))
            }
        }
    }

    pub async fn shutdown(self) {
        #[cfg(target_os = "linux")]
        if let Some(watcher) = self.watcher {
            if let Err(error) = watcher.shutdown().await {
                log::error!("storage watcher stopped: {error}");
            }
        }
        self.core.stop_core().await;
        if let Some(login) = self.login {
            if let Err(error) = login.shutdown().await {
                log::error!("Local approval service stopped: {error}");
            }
        }
        if !self.host.is_finished() {
            let _ = self.host.await;
        }
    }
}

fn capabilities() -> Vec<String> {
    let mut capabilities = vec![
        "PEER_DISCOVERY".into(),
        "FILE_BROWSER".into(),
        "IMAGE_LIBRARY".into(),
        "AUDIO_LIBRARY".into(),
        "VIDEO_LIBRARY".into(),
        "DOCUMENT_LIBRARY".into(),
        "FILE_EXPORT".into(),
        "MEDIA_SCAN".into(),
        "NOTES".into(),
        "FEEDS".into(),
        "BACKGROUND_TIMER".into(),
        "POMODORO".into(),
        "APP_UPDATES".into(),
    ];
    capabilities.extend(
        plain_rs::services::files::media_action_io::capabilities()
            .iter()
            .map(|value| (*value).to_owned()),
    );
    #[cfg(target_os = "linux")]
    {
        capabilities.push("APP_RELAUNCH".into());
        if !plain_rs::platform::linux::samba::detect_systemd_service_name().is_empty() {
            capabilities.push("LAN_SHARE".into());
        }
        if has_command("lsblk") {
            capabilities.push("DISK_MANAGER".into());
        }
    }
    if has_command("libreoffice") || has_command("soffice") {
        capabilities.push("DOC_PREVIEW".into());
    }
    if login::available() {
        capabilities.push("LOCAL_LOGIN_APPROVAL".into());
    }
    capabilities
}

fn has_command(command: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|path| {
            let executable = path.join(command);
            executable.is_file() || cfg!(windows) && executable.with_extension("exe").is_file()
        })
    })
}

#[cfg(test)]
#[path = "../tests/unit/service.rs"]
mod tests;
