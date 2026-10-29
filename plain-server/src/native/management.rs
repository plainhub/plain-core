use crate::assembly::server::ServerState;
use plain_rs::events::WsEvent;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub use crate::services::http_server::HttpServerState;
pub use crate::services::settings::update_download::AppUpdateDownload;
pub use crate::services::settings::updates::AppUpdateStatus;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSession {
    pub client_id: String,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWebLoginRequest {
    pub request_id: String,
    pub client_id: String,
    pub client_ip: String,
    pub browser_name: String,
    pub browser_version: String,
    pub os_name: String,
    pub os_version: String,
    pub verification_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWebLoginRequests {
    pub requests: Vec<NativeWebLoginRequest>,
    pub next_after: Option<String>,
}

#[derive(Clone)]
pub struct NativeManagement {
    state: ServerState,
}

impl NativeManagement {
    pub fn peer_notifications(&self) -> Result<serde_json::Value, String> {
        self.state.native_client_id()?;
        Ok(self.state.management.notifications.snapshot())
    }

    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }

    pub async fn lan_ipv4_addresses(&self) -> Result<Vec<String>, String> {
        self.state.native_client_id()?;
        crate::platform::network::lan_ipv4_addresses().await
    }

    pub fn complete_onboarding(&self) -> Result<bool, String> {
        self.state.native_client_id()?;
        let changed = self
            .state
            .store
            .get()
            .map_err(|e| e.to_string())?
            .set_json("onboarding_completed", true)
            .map_err(|e| e.to_string())?;
        if changed {
            self.state.prefs.notify_changed();
            let _ = self
                .state
                .events
                .send(WsEvent::broadcast("PREFS_UPDATED", "{}".into()));
        }
        Ok(changed)
    }

    pub async fn toggle_notification_app(&self, package_name: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        crate::services::settings::preferences::Preferences {
            prefs: &self.state.prefs,
            events: Some(&self.state.events),
            server: Some(&self.state),
        }
        .action(plain_rs::prefs::settings::Action::ToggleNotificationApp(
            package_name,
        ))
        .await
        .map_err(|error| error.to_string())
    }

    pub async fn restart_http_server(
        &self,
        http_port: Option<u16>,
        https_port: Option<u16>,
    ) -> Result<HttpServerState, String> {
        self.state.native_client_id()?;
        let receipt = crate::services::http_server::restart(&self.state, http_port, https_port)?;
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = stop.wait_for(|stopped| *stopped) => Err("Rust core stopped".into()),
            result = self.state.http_server.wait_finished(receipt.revision) => result,
        }
    }

    pub async fn ble_incoming(
        &self,
        remote_host: String,
        characteristic_uuid: String,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.state.native_client_id()?;
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = stop.wait_for(|stopped| *stopped) => Err("BLE core stopped".into()),
            result = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                crate::services::discovery::ble_http::dispatch_incoming(
                    self.state.clone(),
                    remote_host,
                    characteristic_uuid,
                    &bytes,
                ),
            ) => result
                .map_err(|_| "BLE exchange timed out".to_owned())?
                .map_err(|error| error.to_string()),
        }
    }

    #[cfg(feature = "http_transport")]
    pub async fn check_http(&self) -> Result<bool, String> {
        self.state.native_client_id()?;
        let Some((port, _)) = self.state.public_ports().await else {
            return Ok(false);
        };
        reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|error| error.to_string())?
            .get(format!("http://127.0.0.1:{port}/health"))
            .send()
            .await
            .map(|response| response.status() == reqwest::StatusCode::OK)
            .map_err(|error| error.to_string())
    }

    pub async fn regenerate_tls_certificate(&self) -> Result<Vec<u8>, String> {
        self.certificate(json!({"action":"generate"})).await
    }

    pub async fn import_tls_certificate_pem(
        &self,
        certificate_pem: String,
        private_key_pem: String,
    ) -> Result<Vec<u8>, String> {
        self.certificate(json!({"action":"import","certificatePem":certificate_pem,"privateKeyPem":private_key_pem})).await
    }

    pub async fn import_tls_certificate_pkcs12(
        &self,
        data: Vec<u8>,
        password: String,
    ) -> Result<Vec<u8>, String> {
        self.certificate(json!({"action":"importPkcs12","data":plain_rs::utils::base64::base64_encode(&data),"password":password})).await
    }

    async fn certificate(&self, command: serde_json::Value) -> Result<Vec<u8>, String> {
        self.state.native_client_id()?;
        let signature = crate::services::tls_identity::server_action(&self.state, command).await?;
        self.content_changed();
        Ok(signature)
    }

    pub fn create_session(&self, name: String) -> Result<NativeSession, String> {
        self.state.native_client_id()?;
        let session = crate::services::sessions::create(&self.state, name)
            .map_err(|error| error.to_string())?;
        self.content_changed();
        Ok(NativeSession {
            client_id: session.client_id,
            token: session.token,
        })
    }

    pub async fn confirm_web_login(
        &self,
        request_id: String,
        allowed: bool,
    ) -> Result<bool, String> {
        self.state.native_client_id()?;
        crate::http::auth::ws_login::Confirmation::new(self.state.clone())
            .decide(&request_id, allowed)
            .await?;
        Ok(true)
    }

    pub fn web_login_requests(
        &self,
        after: Option<String>,
    ) -> Result<NativeWebLoginRequests, String> {
        self.state.native_client_id()?;
        Ok(self
            .state
            .login_attempts
            .lock()
            .map_err(|_| "Login state unavailable")?
            .pending_requests(after.as_deref()))
    }

    pub async fn stage_app_restore(&self, path: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        let path = self
            .state
            .file_platform
            .resolve_reference(&path)
            .map_err(|error| error.to_string())?;
        crate::services::backups::restore::stage(
            path,
            self.state.directory.clone(),
            self.state.cache.root().to_owned(),
            self.state.bridge.clone(),
            self.state.stop.clone(),
        )
        .await?;
        Ok(true)
    }

    pub async fn export_app_backup(&self) -> Result<String, String> {
        self.state.native_client_id()?;
        crate::services::backups::export(
            self.state.db.clone(),
            self.state.prefs.clone(),
            self.state.directory.clone(),
            self.state.cache.root().to_owned(),
            self.state.stop.clone(),
        )
        .await
    }

    pub async fn create_crash_report(&self, report: String) -> Result<String, String> {
        self.state.native_client_id()?;
        crate::services::crash_reports::write(&self.state, report).await
    }

    pub fn verify_adb_token(&self, token: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        plain_rs::prefs::security::verify_adb_token(&self.state.prefs, &token)
            .map_err(|error| error.to_string())
    }

    pub async fn start_app_update_download(&self) -> Result<AppUpdateDownload, String> {
        self.state.native_client_id()?;
        let result = crate::services::settings::update_download::execute(
            &self.state,
            crate::services::settings::update_download::Command::Start,
        )
        .await
        .map_err(|error| error.to_string())?;
        self.content_changed();
        Ok(result)
    }

    pub async fn cancel_app_update_download(&self) -> Result<AppUpdateDownload, String> {
        self.state.native_client_id()?;
        let result = crate::services::settings::update_download::execute(
            &self.state,
            crate::services::settings::update_download::Command::Cancel,
        )
        .await
        .map_err(|error| error.to_string())?;
        self.content_changed();
        Ok(result)
    }

    pub async fn check_app_update(&self, automatic: bool) -> Result<AppUpdateStatus, String> {
        self.state.native_client_id()?;
        crate::services::settings::updates::check(&self.state, automatic)
            .await
            .map_err(|error| error.to_string())
    }

    fn content_changed(&self) {
        let _ = self
            .state
            .events
            .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
    }
}

#[cfg(test)]
#[path = "../../tests/unit/native/management.rs"]
mod tests;

mod casting;
mod discovery;
mod files;
mod peers;
mod preferences;
