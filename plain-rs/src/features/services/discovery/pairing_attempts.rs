use crate::features::discovery::nearby_devices::DiscoveryMethod;
use crate::features::services::discovery::pairing_runtime;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::protocols::http_client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Mode {
    Chat,
    Manage,
}

struct Attempt {
    mode: Mode,
    cancel: watch::Sender<bool>,
    trusted: Option<watch::Sender<bool>>,
}
#[derive(Default)]
pub struct Attempts(Mutex<HashMap<String, Attempt>>);
impl Attempts {
    pub fn mode(&self, peer_id: &str) -> Option<Mode> {
        self.0.lock().unwrap().get(peer_id).map(|a| a.mode)
    }
    pub fn contains(&self, peer_id: &str) -> bool {
        self.0.lock().unwrap().contains_key(peer_id)
    }
    fn begin(&self, peer_id: &str, mode: Mode) -> Result<watch::Receiver<bool>> {
        let mut attempts = self.0.lock().unwrap();
        ensure!(
            attempts.contains_key(peer_id) || attempts.len() < 128,
            "Pairing capacity exceeded"
        );
        let (cancel, stopped) = watch::channel(false);
        if let Some(previous) = attempts.insert(
            peer_id.into(),
            Attempt {
                mode,
                cancel,
                trusted: None,
            },
        ) {
            previous.cancel.send_replace(true);
        }
        Ok(stopped)
    }
    pub fn request_confirmation(&self, peer_id: &str) -> Result<watch::Receiver<bool>> {
        let mut attempts = self.0.lock().unwrap();
        let attempt = attempts
            .get_mut(peer_id)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing expired"))?;
        let (trusted, response) = watch::channel(false);
        attempt.trusted = Some(trusted);
        Ok(response)
    }
    pub fn confirm(&self, peer_id: &str) -> Result<()> {
        let mut attempts = self.0.lock().unwrap();
        let trusted = attempts
            .get_mut(peer_id)
            .and_then(|a| a.trusted.take())
            .ok_or_else(|| plain_libs::anyhow::anyhow!("No pending pairing confirmation"))?;
        trusted.send_replace(true);
        Ok(())
    }
    pub fn awaiting_confirmation(&self, peer_id: &str) -> bool {
        self.0
            .lock()
            .unwrap()
            .get(peer_id)
            .is_some_and(|a| a.trusted.is_some())
    }
    pub fn cancel_peer(&self, state: &CoreRuntime, peer_id: &str) {
        if let Some(attempt) = self.0.lock().unwrap().remove(peer_id) {
            attempt.cancel.send_replace(true);
        }
        state.pairing.cancel(peer_id, None);
    }
}

pub async fn cancel(state: &CoreRuntime, peer_id: &str) -> Result<bool> {
    let attempt = {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        state
            .pairing_runtime
            .attempts
            .0
            .lock()
            .unwrap()
            .remove(peer_id)
    };
    let Some(attempt) = attempt else {
        return Ok(true);
    };
    attempt.cancel.send_replace(true);
    if attempt.mode == Mode::Chat {
        pairing_runtime::execute(
            state,
            pairing_runtime::Request::Cancel {
                id: peer_id.into(),
                generation: None,
            },
        )
        .await?;
    } else {
        pairing_runtime::event(
            state,
            crate::features::chat::events::WS_PAIRING_CANCELED,
            plain_libs::serde_json::json!({"deviceId":peer_id,"deviceName":""}),
        );
    }
    Ok(true)
}

pub async fn start(
    state: &CoreRuntime,
    peer_id: String,
    mode: Mode,
    password: Option<String>,
    scope: Option<crate::infrastructure::host::request_scope::NativeRequestScope>,
) -> Result<bool> {
    ensure!(
        !peer_id.is_empty() && peer_id.len() <= 256,
        "Invalid peer identity"
    );
    ensure!(
        password.as_ref().is_none_or(|p| p.len() <= 4096),
        "Password exceeds limit"
    );
    ensure!(
        mode == Mode::Manage || password.is_none(),
        "Chat pairing does not accept a password"
    );
    let qr = if mode == Mode::Chat {
        state.pairing_runtime.qr.pending(&peer_id)?
    } else {
        None
    };
    let target = if qr.is_none() {
        Some(resolve(state, &peer_id, mode)?)
    } else {
        None
    };
    let mut canceled = {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        state.pairing.cancel(&peer_id, None);
        state.pairing_runtime.attempts.begin(&peer_id, mode)?
    };
    let state = state.clone();
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let mut events = state.events.subscribe();
        let work = async {
            if let Some(qr) = qr {
                return super::pairing_qr::respond(&state, qr).await;
            }
            let target = target.unwrap();
            match mode {
                Mode::Manage => super::pairing_login::run(&state, &peer_id, target, password).await,
                Mode::Chat => {
                    let result = pairing_runtime::start_device(
                        &state,
                        target.target,
                        target.ips,
                        target.methods,
                        None,
                    )
                    .await?;
                    if result["ticket"].is_null() {
                        return Ok(());
                    }
                    while let Ok(event) = events.recv().await {
                        if !matches!(
                            event.event_type,
                            "PAIRING_COMPLETED" | "PAIRING_FAILED" | "PAIRING_CANCELED"
                        ) {
                            continue;
                        }
                        let value: plain_libs::serde_json::Value =
                            plain_libs::serde_json::from_str(&event.payload)?;
                        if value["deviceId"] == peer_id {
                            return Ok(());
                        }
                    }
                    plain_libs::anyhow::bail!("Core stopped")
                }
            }
        };
        let scope_closed = async {
            match scope {
                Some(scope) => scope.wait_closed().await,
                None => std::future::pending::<()>().await,
            }
        };
        let deadline = match mode {
            Mode::Chat => crate::features::peers::pairing::sessions::RESPONSE_TIMEOUT,
            Mode::Manage => Duration::from_secs(180),
        };
        let result = tokio::select! { biased;
            _ = scope_closed => { let _ = cancel(&state, &peer_id).await; return; },
            _ = canceled.changed() => return,
            _ = stop.changed() => Err(plain_libs::anyhow::anyhow!("Core stopped")),
            result = tokio::time::timeout(deadline, work) => result.unwrap_or_else(|_| Err(plain_libs::anyhow::anyhow!("Pairing timed out"))),
        };
        {
            let _commit = state.pairing_runtime.commit.lock().unwrap();
            state
                .pairing_runtime
                .attempts
                .0
                .lock()
                .unwrap()
                .remove(&peer_id);
        }
        if let Err(error) = result {
            state.pairing.cancel(&peer_id, None);
            pairing_runtime::event(
                &state,
                crate::features::chat::events::WS_PAIRING_FAILED,
                plain_libs::serde_json::json!({"deviceId":peer_id,"deviceName":"","error":error.to_string()}),
            );
        }
    });
    Ok(true)
}

pub(super) struct Resolved {
    pub target: crate::features::peers::pairing::sessions::Target,
    pub ips: Vec<String>,
    pub methods: Vec<DiscoveryMethod>,
    pub device_type: crate::features::chat::enums::DeviceType,
    pub url: Option<http_client::Url>,
}
pub fn nearby_management_address(state: &CoreRuntime, id: &str) -> Result<http_client::Url> {
    ensure!(
        state.nearby_devices.contains(id),
        "Nearby device unavailable"
    );
    resolve(state, id, Mode::Manage)?
        .url
        .ok_or_else(|| plain_libs::anyhow::anyhow!("No management address available"))
}

fn resolve(state: &CoreRuntime, id: &str, mode: Mode) -> Result<Resolved> {
    let nearby = state
        .nearby_devices
        .snapshot()
        .devices
        .into_iter()
        .find(|d| d.id == id);
    let saved = crate::infrastructure::db::chat_store::peers::get(&state.db, id)?;
    let name = nearby
        .as_ref()
        .map(|d| d.name.clone())
        .or_else(|| saved.as_ref().map(|p| p.name.clone()))
        .unwrap_or_default();
    let ips = nearby
        .as_ref()
        .map(|d| d.ips.clone())
        .or_else(|| {
            saved
                .as_ref()
                .map(|p| p.ip.split(',').map(str::to_owned).collect())
        })
        .unwrap_or_default();
    let port = nearby
        .as_ref()
        .map(|d| d.port)
        .or_else(|| saved.as_ref().and_then(|p| p.port))
        .unwrap_or(0);
    let methods = nearby
        .as_ref()
        .map(|d| d.discovery_methods.clone())
        .unwrap_or_else(|| vec![DiscoveryMethod::Lan]);
    let device_type = nearby
        .as_ref()
        .map(|d| d.device_type)
        .or_else(|| saved.as_ref().map(|p| p.device_type))
        .unwrap_or(crate::features::chat::enums::DeviceType::Unknown);
    let url = if mode == Mode::Manage {
        let ip = crate::infrastructure::transport::lan::best_host(
            &ips,
            &crate::infrastructure::transport::lan::local_interfaces(),
        );
        ensure!(
            !ip.is_empty() && port != 0,
            "No management address available"
        );
        Some(http_client::Url::parse(&format!(
            "https://{}:{port}",
            if ip.contains(':') {
                format!("[{ip}]")
            } else {
                ip
            }
        ))?)
    } else {
        None
    };
    let public_key = if mode == Mode::Chat {
        saved
            .as_ref()
            .map(|peer| peer.public_key.clone())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| plain_libs::anyhow::anyhow!("A trusted pairing QR is required"))?
    } else {
        nearby
            .as_ref()
            .map(|device| device.signature_public_key.clone())
            .filter(|key| !key.is_empty())
            .or_else(|| saved.as_ref().map(|peer| peer.public_key.clone()))
            .unwrap_or_default()
    };
    Ok(Resolved {
        target: crate::features::peers::pairing::sessions::Target {
            device_id: id.into(),
            device_name: name,
            device_public_key: public_key,
            device_ip: String::new(),
            device_port: port,
        },
        ips,
        methods,
        device_type,
        url,
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing_attempts.rs"]
mod tests;
