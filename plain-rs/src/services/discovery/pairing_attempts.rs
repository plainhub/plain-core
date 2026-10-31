use anyhow::{Result, ensure};
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::discovery::pairing_runtime;
use plain_rs::services::discovery::pairing_runtime::event;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Mode {
    #[default]
    Chat,
    Manage,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub device_id: Option<String>,
    pub generation: String,
    pub mode: Mode,
    pub phase: String,
    pub error: Option<String>,
    #[serde(default)]
    pub verification_code: Option<String>,
}
impl State {
    pub fn terminal(&self) -> bool {
        matches!(self.phase.as_str(), "SUCCEEDED" | "FAILED" | "CANCELLED")
    }
}
struct Attempt {
    state: State,
    host: Option<String>,
    updated: Instant,
    started: u64,
    cancel: watch::Sender<bool>,
    password: mpsc::Sender<String>,
}
#[derive(Default)]
pub struct Attempts(
    Mutex<HashMap<String, Attempt>>,
    std::sync::atomic::AtomicU64,
);

impl Attempts {
    pub fn states(&self) -> Vec<State> {
        let mut attempts = self.0.lock().unwrap();
        attempts
            .retain(|_, a| !a.state.terminal() || a.updated.elapsed() < Duration::from_secs(300));
        let mut entries = attempts.values().collect::<Vec<_>>();
        entries.sort_by_key(|a| a.started);
        entries.into_iter().map(|a| a.state.clone()).collect()
    }
    pub fn get(&self, generation: &str) -> Option<State> {
        self.0
            .lock()
            .unwrap()
            .get(generation)
            .map(|a| a.state.clone())
    }
    pub fn begin(
        &self,
        context: Option<&CoreRuntime>,
        id: Option<String>,
        host: Option<String>,
        mode: Mode,
    ) -> Result<(State, watch::Receiver<bool>, mpsc::Receiver<String>)> {
        let mut attempts = self.0.lock().unwrap();
        attempts
            .retain(|_, a| !a.state.terminal() || a.updated.elapsed() < Duration::from_secs(300));
        for a in attempts.values_mut().filter(|a| {
            !a.state.terminal()
                && ((id.is_some() && a.state.device_id == id) || (host.is_some() && a.host == host))
        }) {
            a.cancel.send_replace(true);
            a.state.phase = "CANCELLED".into();
            a.updated = Instant::now();
            if let Some(state) = context {
                event(
                    state,
                    "PAIRING_STATE_CHANGED",
                    serde_json::to_value(&a.state).unwrap(),
                );
            }
        }
        if attempts.len() >= 128 {
            let oldest = attempts
                .iter()
                .filter(|(_, a)| a.state.terminal())
                .min_by_key(|(_, a)| a.updated)
                .map(|(id, _)| id.clone());
            if let Some(oldest) = oldest {
                attempts.remove(&oldest);
            }
        }
        ensure!(attempts.len() < 128, "Pairing capacity exceeded");
        let state = State {
            device_id: id,
            generation: uuid::Uuid::new_v4().to_string(),
            mode,
            phase: "STARTING".into(),
            error: None,
            verification_code: None,
        };
        let (cancel, stopped) = watch::channel(false);
        let (password, passwords) = mpsc::channel(1);
        attempts.insert(
            state.generation.clone(),
            Attempt {
                state: state.clone(),
                host,
                updated: Instant::now(),
                started: self.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                cancel,
                password,
            },
        );
        if let Some(context) = context {
            event(
                context,
                "PAIRING_STATE_CHANGED",
                serde_json::to_value(&state).unwrap(),
            );
        }
        Ok((state, stopped, passwords))
    }
    pub fn set_verification_code(&self, state: &CoreRuntime, generation: &str, code: &str) {
        let mut attempts = self.0.lock().unwrap();
        if let Some(attempt) = attempts
            .get_mut(generation)
            .filter(|attempt| !attempt.state.terminal())
        {
            attempt.state.verification_code = Some(code.into());
            event(
                state,
                "PAIRING_STATE_CHANGED",
                serde_json::to_value(&attempt.state).unwrap(),
            );
        }
    }
    pub fn transition(
        &self,
        state: &CoreRuntime,
        generation: &str,
        phase: &str,
        error: Option<&str>,
        id: Option<&str>,
    ) -> bool {
        let mut attempts = self.0.lock().unwrap();
        let Some(a) = attempts.get_mut(generation).filter(|a| !a.state.terminal()) else {
            return false;
        };
        a.state.phase = phase.into();
        a.state.error = error.map(str::to_owned);
        if let Some(id) = id {
            a.state.device_id = Some(id.into());
        }
        a.updated = Instant::now();
        event(
            state,
            "PAIRING_STATE_CHANGED",
            serde_json::to_value(&a.state).unwrap(),
        );
        true
    }
    pub fn claim_identity(
        &self,
        context: Option<&CoreRuntime>,
        generation: &str,
        id: &str,
    ) -> Result<()> {
        let mut attempts = self.0.lock().unwrap();
        let started = attempts
            .get(generation)
            .filter(|a| !a.state.terminal())
            .ok_or_else(|| anyhow::anyhow!("cancelled"))?
            .started;
        ensure!(
            !attempts
                .values()
                .any(|a| a.state.device_id.as_deref() == Some(id) && a.started > started),
            "Pairing superseded"
        );
        for a in attempts.values_mut().filter(|a| {
            a.state.generation != generation
                && a.state.device_id.as_deref() == Some(id)
                && !a.state.terminal()
        }) {
            a.cancel.send_replace(true);
            a.state.phase = "CANCELLED".into();
            a.updated = Instant::now();
            if let Some(state) = context {
                event(
                    state,
                    "PAIRING_STATE_CHANGED",
                    serde_json::to_value(&a.state).unwrap(),
                );
            }
        }
        attempts.get_mut(generation).unwrap().state.device_id = Some(id.into());
        Ok(())
    }
    pub fn cancel_peer(&self, state: &CoreRuntime, id: &str) {
        let generations = self
            .states()
            .into_iter()
            .filter(|a| a.device_id.as_deref() == Some(id) && !a.terminal())
            .map(|a| a.generation)
            .collect::<Vec<_>>();
        for generation in generations {
            self.cancel(state, &generation);
        }
        state.pairing.cancel(id, None);
    }
    fn submit(
        &self,
        context: Option<&CoreRuntime>,
        generation: &str,
        id: Option<&str>,
        host: Option<&str>,
        mode: Option<Mode>,
        password: String,
    ) -> Result<State> {
        let mut attempts = self.0.lock().unwrap();
        let a = attempts
            .get_mut(generation)
            .ok_or_else(|| anyhow::anyhow!("Unknown pairing generation"))?;
        ensure!(
            a.state.mode == Mode::Manage && a.state.phase == "PASSWORD_REQUIRED",
            "Pairing is not waiting for a password"
        );
        ensure!(
            id.is_none_or(|id| a.state.device_id.as_deref() == Some(id))
                && host.is_none_or(|host| a.host.as_deref() == Some(host))
                && mode.is_none_or(|mode| mode == a.state.mode),
            "Pairing target or mode changed"
        );
        a.password
            .try_send(password)
            .map_err(|_| anyhow::anyhow!("Password already submitted"))?;
        a.state.phase = "AUTHENTICATING".into();
        a.updated = Instant::now();
        if let Some(state) = context {
            event(
                state,
                "PAIRING_STATE_CHANGED",
                serde_json::to_value(&a.state).unwrap(),
            );
        }
        Ok(a.state.clone())
    }
    fn cancel(&self, state: &CoreRuntime, generation: &str) -> Option<State> {
        let mut attempts = self.0.lock().unwrap();
        let a = attempts
            .get_mut(generation)
            .filter(|a| !a.state.terminal())?;
        a.cancel.send_replace(true);
        a.state.phase = "CANCELLED".into();
        a.updated = Instant::now();
        event(
            state,
            "PAIRING_STATE_CHANGED",
            serde_json::to_value(&a.state).unwrap(),
        );
        Some(a.state.clone())
    }
}

pub async fn cancel(state: &CoreRuntime, generation: &str) -> Result<bool> {
    let cancelled = {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        state.pairing_runtime.attempts.cancel(state, generation)
    };
    if let Some(a) = cancelled {
        if a.mode == Mode::Chat {
            pairing_runtime::execute(
                state,
                pairing_runtime::Request::Cancel {
                    id: a.device_id.unwrap_or_default(),
                    generation: Some(generation.into()),
                },
            )
            .await?;
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

pub async fn start(
    state: &CoreRuntime,
    id: Option<String>,
    host: Option<String>,
    mode: Option<Mode>,
    password: Option<String>,
    generation: Option<String>,
    scope: Option<plain_rs::host::request_scope::NativeRequestScope>,
) -> Result<State> {
    ensure!(
        password.as_ref().is_none_or(|p| p.len() <= 4096),
        "Password exceeds limit"
    );
    if let Some(generation) = generation {
        return state.pairing_runtime.attempts.submit(
            Some(state),
            &generation,
            id.as_deref(),
            host.as_deref(),
            mode,
            password.ok_or_else(|| anyhow::anyhow!("Password required"))?,
        );
    }
    ensure!(
        id.as_ref()
            .is_none_or(|id| !id.is_empty() && id.len() <= 256),
        "Invalid peer identity"
    );
    ensure!(id.is_some() || host.is_some(), "Pairing target required");
    let mode = mode.unwrap_or_default();
    ensure!(
        mode == Mode::Manage || password.is_none(),
        "Chat pairing does not accept a password"
    );
    let target = resolve(state, id.as_deref(), host.as_deref(), mode)?;
    let (initial, mut cancelled, passwords) = {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        if let Some(id) = id.as_deref() {
            state.pairing.cancel(id, None);
        }
        state
            .pairing_runtime
            .attempts
            .begin(Some(state), id, host, mode)?
    };
    let state = state.clone();
    let generation = initial.generation.clone();
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        let work = async {
            match mode {
                Mode::Manage => {
                    plain_rs::services::discovery::pairing_login::run(
                        &state,
                        &generation,
                        target,
                        password,
                        passwords,
                    )
                    .await
                }
                Mode::Chat => {
                    let result = pairing_runtime::start_device(
                        &state,
                        target.target,
                        target.ips,
                        target.methods,
                        Some(generation.clone()),
                    )
                    .await?;
                    ensure!(
                        !result["ticket"].is_null(),
                        "No pairing transport available"
                    );
                    let mut events = state.events.subscribe();
                    loop {
                        if state
                            .pairing_runtime
                            .attempts
                            .get(&generation)
                            .is_none_or(|s| s.terminal())
                        {
                            return Ok(());
                        }
                        let _ = events.recv().await;
                    }
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
            Mode::Chat => plain_rs::peers::pairing::sessions::RESPONSE_TIMEOUT,
            Mode::Manage => Duration::from_secs(180),
        };
        let result = tokio::select! { biased;
            _ = scope_closed => { let _ = cancel(&state, &generation).await; return; },
            _ = cancelled.changed() => return,
            _ = stop.changed() => Err(anyhow::anyhow!("Core stopped")),
            result = tokio::time::timeout(deadline, work) => result.unwrap_or_else(|_| Err(anyhow::anyhow!("Pairing timed out"))),
        };
        if let Err(error) = result {
            let _commit = state.pairing_runtime.commit.lock().unwrap();
            if let Some(id) = state
                .pairing_runtime
                .attempts
                .get(&generation)
                .and_then(|a| a.device_id)
            {
                state.pairing.cancel(&id, Some(&generation));
            }
            state.pairing_runtime.attempts.transition(
                &state,
                &generation,
                "FAILED",
                Some(&error.to_string()),
                None,
            );
        }
    });
    Ok(initial)
}

pub(super) struct Resolved {
    pub target: plain_rs::peers::pairing::sessions::Target,
    pub ips: Vec<String>,
    pub methods: Vec<String>,
    pub device_type: plain_rs::chat::enums::DeviceType,
    pub url: Option<reqwest::Url>,
}
fn resolve(
    state: &CoreRuntime,
    id: Option<&str>,
    host: Option<&str>,
    mode: Mode,
) -> Result<Resolved> {
    let nearby = state
        .nearby_devices
        .snapshot()
        .devices
        .into_iter()
        .find(|d| Some(d.id.as_str()) == id);
    let saved = id
        .map(|id| plain_rs::db::chat_store::peers::get(&state.db, id))
        .transpose()?
        .flatten();
    let name = nearby
        .as_ref()
        .map(|d| d.name.clone())
        .or_else(|| saved.as_ref().map(|p| p.name.clone()))
        .unwrap_or_default();
    let mut ips = nearby
        .as_ref()
        .map(|d| d.ips.clone())
        .or_else(|| {
            saved
                .as_ref()
                .map(|p| p.ip.split(',').map(str::to_owned).collect())
        })
        .unwrap_or_default();
    let mut port = nearby
        .as_ref()
        .map(|d| d.port)
        .or_else(|| saved.as_ref().and_then(|p| p.port))
        .unwrap_or(0);
    let mut methods = nearby
        .as_ref()
        .map(|d| d.discovery_methods.clone())
        .unwrap_or_else(|| vec!["LAN".into()]);
    let device_type = nearby
        .as_ref()
        .and_then(|d| d.device_type.parse().ok())
        .or_else(|| saved.as_ref().map(|p| p.device_type))
        .unwrap_or(plain_rs::chat::enums::DeviceType::Unknown);
    let url = if let Some(host) = host {
        ensure!(host.len() <= 2048, "Invalid peer host");
        let raw = if host.contains("://") {
            host.to_owned()
        } else {
            format!("https://{host}")
        };
        let url = reqwest::Url::parse(&raw)?;
        ensure!(
            matches!(url.scheme(), "https" | "http")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none(),
            "Invalid peer host"
        );
        ips = vec![url.host_str().unwrap().trim_matches(['[', ']']).to_owned()];
        port = url.port_or_known_default().unwrap();
        methods = vec!["LAN".into()];
        Some(url)
    } else if mode == Mode::Manage {
        let ip = plain_rs::transport::lan::best_host(
            &ips,
            &plain_rs::transport::lan::local_interfaces(),
        );
        ensure!(
            !ip.is_empty() && port != 0,
            "No management address available"
        );
        Some(reqwest::Url::parse(&format!(
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
    ensure!(
        mode != Mode::Chat || id.is_some(),
        "Chat pairing requires an expected peer identity"
    );
    Ok(Resolved {
        target: plain_rs::peers::pairing::sessions::Target {
            device_id: id.unwrap_or_default().into(),
            device_name: name,
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
#[path = "../../../tests/unit/services/discovery/pairing_attempts.rs"]
mod tests;
