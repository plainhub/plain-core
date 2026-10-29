use super::server::{PublicServer, ServerState};
use tokio::sync::watch;

impl ServerState {
    pub(crate) async fn start_public_locked(
        &self,
        http: u16,
        https: u16,
        cert: Vec<u8>,
        key: Vec<u8>,
    ) -> Result<(u16, u16), String> {
        self.native_client_id()?;
        let mut guard = self.public_server.lock().await;
        if guard.is_some() {
            return Err("Public HTTP server is already running".into());
        }
        let (stop, receiver) = watch::channel(false);
        let router = crate::http::routes::public::build_router(self.clone(), receiver.clone());
        let started = async {
            let fingerprint = crate::services::tls_identity::certificate_sha256(&cert)?;
            let listeners = crate::http::listeners::HttpListeners::start(router, http, https, cert, key).await?;
            Ok::<_, String>((listeners, fingerprint))
        }.await;
        let (listeners, tls_certificate_sha256) = match started {
            Ok(started) => started,
            Err(error) => {
                self.http_server.failed(error.clone());
                self.publish_http_status();
                return Err(error);
            }
        };
        let ports = (listeners.http_port, listeners.https_port);
        if let Err(error) = crate::services::settings::preferences::Preferences::from_server(self)
            .patch(plain_rs::prefs::settings::SettingsPatch {
                http_port: Some(i32::from(ports.0)),
                https_port: Some(i32::from(ports.1)),
                ..Default::default()
            })
            .await
        {
            let error = error.to_string();
            listeners.shutdown().await;
            self.http_server.failed(error.clone());
            self.publish_http_status();
            return Err(error);
        }
        self.http_server.started(ports);
        {
            let _control = self.peer_status.control.lock().await;
            self.peer_status
                .public_active
                .store(true, std::sync::atomic::Ordering::SeqCst);
            self.peer_status.outgoing.start(self.clone());
        }
        let failure = listeners.failures();
        let discovery =
            crate::assembly::lifecycle::PublicDiscovery::start(self.clone(), receiver.clone());
        *guard = Some(PublicServer {
            tls_certificate_sha256,
            discovery,
            listeners,
            stop,
        });
        crate::assembly::lifecycle::monitor(self.clone(), failure, receiver);
        drop(guard);
        if let Err(error) = crate::services::casting::receiver::lifecycle::reconcile(self).await {
            log::warn!("DLNA receiver startup: {error}");
        }
        self.publish_http_status();
        Ok(ports)
    }
    pub(crate) async fn http_configuration(
        &self,
    ) -> Result<(u16, u16, Vec<u8>, Vec<u8>), String> {
        let settings =
            plain_rs::prefs::settings::snapshot(&self.prefs).map_err(|error| error.to_string())?;
        let http = u16::try_from(settings.http_port).map_err(|error| error.to_string())?;
        let https = u16::try_from(settings.https_port).map_err(|error| error.to_string())?;
        let identity = self
            .tls_identity
            .read()
            .map_err(|_| "TLS identity configuration unavailable")?
            .clone();
        let (cert, key) = tokio::task::spawn_blocking(move || identity.identity())
            .await
            .map_err(|error| error.to_string())??;
        Ok((http, https, cert, key))
    }

    pub(crate) async fn restart_public_locked(&self) -> Result<(u16, u16), String> {
        self.native_client_id()?;
        let (http, https, cert, key) = self.http_configuration().await?;
        {
            let mut server = self.public_server.lock().await.take();
            crate::assembly::lifecycle::stop_locked(self, &mut server).await;
            self.http_server.stopped();
        }
        let ports = self.start_public_locked(http, https, cert, key).await?;
        self.mdns
            .update_hostname(self)
            .await
            .map_err(|error| error.to_string())?;
        Ok(ports)
    }
}
