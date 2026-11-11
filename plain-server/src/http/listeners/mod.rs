pub mod bind;
#[cfg(feature = "core")]
pub(crate) mod control;
#[cfg(feature = "core")]
pub(crate) mod lifecycle;
#[cfg(feature = "core")]
mod public_server;

use plain_libs::protocols::http_client;
use plain_libs::axum::{Extension, Router};

use crate::http::ConnectionScheme;
use plain_libs::protocols::https::{self, ServerConfig};
pub use plain_rs::infrastructure::tls_identity::config as tls_config;
use std::{net::SocketAddr, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

/// The supplied router must serve `GET /health` with status 200 for both listeners.
pub struct HttpListeners {
    pub http_port: u16,
    pub https_port: u16,
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    failure: watch::Receiver<Option<String>>,
}

impl HttpListeners {
    pub async fn start(
        router: Router,
        http_port: u16,
        https_port: u16,
        certificate_pem: Vec<u8>,
        private_key_pem: Vec<u8>,
    ) -> Result<Self, String> {
        let tls = tls_config(&certificate_pem, &private_key_pem)?;
        Self::start_with_tls(router, http_port, https_port, tls).await
    }

    pub async fn start_with_tls(
        router: Router,
        http_port: u16,
        https_port: u16,
        tls: ServerConfig,
    ) -> Result<Self, String> {
        let http = bind::bind_listener_fallback(http_port, &bind::HTTP_PORTS)
            .map_err(|e| format!("HTTP bind failed: {e}"))?;
        let https = bind::bind_listener_fallback(https_port, &bind::HTTPS_PORTS)
            .map_err(|e| format!("HTTPS bind failed: {e}"))?;
        let http_port = http.local_addr().map_err(|e| e.to_string())?.port();
        let https_port = https.local_addr().map_err(|e| e.to_string())?.port();
        http.set_nonblocking(true).map_err(|e| e.to_string())?;
        let http = tokio::net::TcpListener::from_std(http).map_err(|e| e.to_string())?;
        let (stop, mut receiver) = watch::channel(false);
        let tls_stop = receiver.clone();
        let app = router
            .clone()
            .layer(Extension(ConnectionScheme("http")))
            .into_make_service_with_connect_info::<SocketAddr>();
        let (failure_sender, failure) = watch::channel(None);
        let http_failure = failure_sender.clone();
        let http_task = tokio::spawn(async move {
            let mut task_guard = ListenerTaskGuard::new("HTTP", http_failure.clone());
            if let Err(error) = plain_libs::axum::serve(http, app)
                .with_graceful_shutdown(async move {
                    let _ = receiver.changed().await;
                })
                .await
            {
                let _ = http_failure.send(Some(format!("HTTP serve failed: {error}")));
            }
            task_guard.completed = true;
        });
        let tls_task = tokio::spawn(async move {
            let mut task_guard = ListenerTaskGuard::new("HTTPS", failure_sender.clone());
            if let Err(error) = https::serve(
                https,
                router.layer(Extension(ConnectionScheme("https"))),
                tls,
                tls_stop,
            )
            .await
            {
                let _ = failure_sender.send(Some(format!("HTTPS serve failed: {error}")));
            }
            task_guard.completed = true;
        });
        let listeners = Self {
            http_port,
            https_port,
            stop,
            tasks: vec![http_task, tls_task],
            failure,
        };
        if let Err(error) = listeners.check_health().await {
            listeners.shutdown().await;
            return Err(error);
        }
        Ok(listeners)
    }

    pub fn failures(&self) -> watch::Receiver<Option<String>> {
        self.failure.clone()
    }

    pub async fn check_health(&self) -> Result<(), String> {
        let client = http_client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_millis(500))
            .build()
            .map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_millis(8_500), async {
            loop {
                if let Some(error) = self.failure.borrow().clone() {
                    return Err(error);
                }
                let mut healthy = true;
                for (scheme, port) in [("http", self.http_port), ("https", self.https_port)] {
                    match client
                        .get(format!("{scheme}://127.0.0.1:{port}/health"))
                        .send()
                        .await
                    {
                        Ok(response) if response.status() == http_client::StatusCode::OK => {}
                        _ => {
                            healthy = false;
                            break;
                        }
                    }
                }
                if healthy {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| "HTTP/HTTPS server readiness timed out".to_string())?
    }

    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        for task in &mut self.tasks {
            if tokio::time::timeout_at(deadline, &mut *task).await.is_err() {
                task.abort();
                let _ = task.await;
            }
        }
    }
}

impl Drop for HttpListeners {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        for task in &self.tasks {
            task.abort();
        }
    }
}

struct ListenerTaskGuard {
    scheme: &'static str,
    failure: watch::Sender<Option<String>>,
    completed: bool,
}

impl ListenerTaskGuard {
    fn new(scheme: &'static str, failure: watch::Sender<Option<String>>) -> Self {
        Self {
            scheme,
            failure,
            completed: false,
        }
    }
}

impl Drop for ListenerTaskGuard {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.failure.send(Some(format!(
                "{} listener task terminated unexpectedly",
                self.scheme
            )));
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/listeners/mod.rs"]
mod tests;
