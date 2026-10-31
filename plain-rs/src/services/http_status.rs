use crate::runtime::Runtime as CoreRuntime;
use plain_rs::types::Long;
use std::sync::Mutex;

#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpServerState {
    pub revision: Long,
    pub restarting: bool,
    pub http_port: i32,
    pub https_port: i32,
    pub error: String,
}

#[derive(Clone, serde::Serialize)]
pub struct HttpStatus {
    pub state: &'static str,
    pub message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HostStatus {
    state: &'static str,
    message: String,
    http_port: i32,
    https_port: i32,
}

impl CoreRuntime {
    pub fn publish_http_status(&self) {
        let mut recovering = self.http_server.host_delivery.lock().unwrap();
        if !self.host.connected() {
            return;
        }
        if let Err(error) = self.host.notify(
            "httpServerStatusChanged",
            serde_json::json!(self.http_server.host_status()),
        ) {
            log::warn!("HTTP server status callback: {error}");
            if *recovering {
                return;
            }
            *recovering = true;
            let state = self.clone();
            tokio::spawn(async move {
                let permit = state.host.reserve_callback().await;
                let mut recovering = state.http_server.host_delivery.lock().unwrap();
                if let Ok(permit) = permit {
                    if let Err(error) = state.host.notify_reserved_callback(
                        "httpServerStatusChanged",
                        serde_json::json!(state.http_server.host_status()),
                        permit,
                    ) {
                        log::warn!("HTTP server status recovery callback: {error}");
                    }
                }
                *recovering = false;
            });
        }
    }
}

#[derive(Default)]
pub struct Runtime {
    state: Mutex<HttpServerState>,
    host_delivery: Mutex<bool>,
    completion: tokio::sync::Notify,
}

impl Runtime {
    pub async fn wait_finished(&self, revision: Long) -> Result<HttpServerState, String> {
        loop {
            let completed = self.completion.notified();
            tokio::pin!(completed);
            completed.as_mut().enable();
            let state = self.snapshot();
            if state.revision.0 != revision.0 {
                return Err("Server configuration was superseded".into());
            }
            if !state.restarting {
                return Ok(state);
            }
            completed.await;
        }
    }

    pub fn snapshot(&self) -> HttpServerState {
        self.state.lock().unwrap().clone()
    }

    pub fn status(&self) -> HttpStatus {
        let state = self.state.lock().unwrap();
        if state.http_port != 0 {
            HttpStatus {
                state: "started",
                message: state.error.clone(),
            }
        } else if !state.error.is_empty() {
            HttpStatus {
                state: "error",
                message: state.error.clone(),
            }
        } else {
            HttpStatus {
                state: "stopped",
                message: String::new(),
            }
        }
    }

    fn host_status(&self) -> HostStatus {
        let state = self.state.lock().unwrap();
        HostStatus {
            state: if state.http_port != 0 {
                "started"
            } else if !state.error.is_empty() {
                "error"
            } else {
                "stopped"
            },
            message: state.error.clone(),
            http_port: state.http_port,
            https_port: state.https_port,
        }
    }

    pub fn notify_snapshot(&self, host: &plain_rs::host::bridge::Host) -> Result<(), String> {
        let _delivery = self.host_delivery.lock().unwrap();
        host.notify(
            "httpServerStatusChanged",
            serde_json::json!(self.host_status()),
        )
    }

    pub fn begin(&self) -> Result<HttpServerState, String> {
        let mut state = self.state.lock().unwrap();
        if state.restarting {
            return Err("HTTP server restart is already running".into());
        }
        state.revision.0 = state
            .revision
            .0
            .checked_add(1)
            .ok_or("HTTP revision exhausted")?;
        state.restarting = true;
        state.error.clear();
        Ok(state.clone())
    }

    pub fn started(&self, ports: (u16, u16)) {
        let mut state = self.state.lock().unwrap();
        state.http_port = i32::from(ports.0);
        state.https_port = i32::from(ports.1);
        state.error.clear();
    }

    pub fn stopped(&self) {
        let mut state = self.state.lock().unwrap();
        state.http_port = 0;
        state.https_port = 0;
        state.error.clear();
    }

    pub fn failed(&self, error: String) {
        let mut state = self.state.lock().unwrap();
        state.http_port = 0;
        state.https_port = 0;
        state.error = error;
    }

    pub fn finish(&self, revision: Long, result: Result<(u16, u16), String>) {
        let mut state = self.state.lock().unwrap();
        if state.revision.0 != revision.0 {
            return;
        }
        state.restarting = false;
        match result {
            Ok((http, https)) => {
                state.http_port = i32::from(http);
                state.https_port = i32::from(https);
                state.error.clear();
            }
            Err(error) => state.error = error,
        }
        drop(state);
        self.completion.notify_waiters();
    }
}
