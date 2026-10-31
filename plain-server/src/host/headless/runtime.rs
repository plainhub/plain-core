use std::sync::Arc;

use crate::server::ContentServer;
use serde_json::Value;

#[cfg(target_os = "linux")]
mod linux;

pub struct Runtime {
    core: Arc<ContentServer>,
    capabilities: Vec<String>,
    multicast: std::sync::Mutex<plain_rs::host::multicast::MulticastLeases>,
    failure: tokio::sync::watch::Sender<Option<String>>,
}

impl Runtime {
    pub fn new(core: Arc<ContentServer>, capabilities: Vec<String>) -> Self {
        Self {
            core,
            capabilities,
            multicast: Default::default(),
            failure: tokio::sync::watch::channel(None).0,
        }
    }

    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, String> {
        #[cfg(target_os = "linux")]
        if linux::handles(method) {
            return linux::dispatch(&self.core, method, params).await;
        }
        match method {
            "httpServerStatusChanged" => {
                let status = self.core.http_status();
                if status.state == "error" {
                    self.failure
                        .send_replace(Some(format!("HTTP server failed: {}", status.message)));
                }
                Ok(Value::Null)
            }
            "systemWebLoginRequested"
            | "systemWebLoginClosed"
            | "systemWebLoginCompleted"
            | "systemWebLoginSnapshot"
            | "peerNotificationsChanged"
            | "peerClipboardChanged"
            | "clipboardHistoryRefresh"
            | "dlnaRendererChanged"
            | "dlnaRendererRefresh"
            | "sharedFolderDownloadsRefresh"
            | "peerTransportsChanged"
            | "appUpdateDownloadChanged"
            | "dlnaSenderChanged"
            | "feedSyncChanged"
            | "audioPlaybackRefresh"
            | "audioLibraryRefresh"
            | "pairingSucceeded"
            | "networkInterfacesChanged"
            | "onlineClientsChanged"
            | "systemWebActivity" => Ok(Value::Null),
            "systemCapabilities" => {
                serde_json::to_value(&self.capabilities).map_err(|error| error.to_string())
            }
            "peerTransportCapabilities" => Ok(serde_json::json!([])),
            "systemPermissionFacts" => plain_rs::host::permission_facts::facts(
                &["WRITE_EXTERNAL_STORAGE".to_owned()],
                params,
            ),
            method if crate::native::audio::handles(method) => {
                self.core.native_audio()?.handle(method, params).await
            }

            "mdnsMulticast" => {
                let result = self
                    .multicast
                    .lock()
                    .map_err(|_| "multicast lease state unavailable")?
                    .without_platform_lock(params)?;
                Ok(Value::Bool(result))
            }

            _ => Err(format!("Unsupported headless host method: {method}")),
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), String> {
        let _multicast = plain_rs::host::multicast::MulticastSession::new(&self.multicast);
        let session = Arc::new(self.core.host_events()?);
        let mut failure = self.failure.subscribe();
        let requests = self.clone();
        tokio::select! {
            result = session.serve(move |method, params| {
                    let requests = requests.clone();
                    async move { requests.dispatch(&method, params).await }
                }) => result,
            changed = failure.wait_for(|value| value.is_some()) => {
                let value = changed.map_err(|_| "HTTP server status channel closed")?;
                Err(value.clone().unwrap())
            },
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/host/headless/runtime.rs"]
mod tests;
