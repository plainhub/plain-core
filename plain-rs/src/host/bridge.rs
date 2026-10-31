use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Notify, Semaphore, mpsc, oneshot, watch};

pub const REQUEST_CAPACITY: usize = 32;

type Reply = Result<Value, String>;
#[derive(Default)]
struct State {
    generation: u64,
    next_id: u64,
    peer_sockets: bool,
    sender: Option<mpsc::Sender<Value>>,
    pending: HashMap<u64, PendingReply>,
}
struct PendingReply {
    reply: oneshot::Sender<Reply>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}
pub struct Host {
    #[cfg(any(test, feature = "test-seams"))]
    pub callback_attempts: std::sync::atomic::AtomicUsize,
    state: Arc<Mutex<State>>,
    ready: Notify,
    capacity: Arc<Semaphore>,
    connection: watch::Sender<u64>,
    startup: plain_rs::host::startup::StartupConfig,
    activity_in_flight: Arc<AtomicBool>,
    pending_invalidations: Mutex<HashSet<&'static str>>,
}
struct ActivityCall(Arc<AtomicBool>);
impl Drop for ActivityCall {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl Default for Host {
    fn default() -> Self {
        Self::new(plain_rs::platform::startup::config())
    }
}
struct Pending {
    state: Arc<Mutex<State>>,
    id: u64,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.state.lock().unwrap().pending.remove(&self.id);
    }
}
fn is_dynamic_property(key: &str) -> bool {
    matches!(
        key,
        "language" | "display.width" | "display.height" | "display.density" | "wifiAwareRunning"
    )
}
impl Host {
    fn new(startup: plain_rs::host::startup::StartupConfig) -> Self {
        Self {
            #[cfg(any(test, feature = "test-seams"))]
            callback_attempts: std::sync::atomic::AtomicUsize::new(0),
            state: Arc::new(Mutex::new(State::default())),
            ready: Notify::new(),
            capacity: Arc::new(Semaphore::new(REQUEST_CAPACITY)),
            connection: watch::channel(0).0,
            startup,
            activity_in_flight: Arc::new(AtomicBool::new(false)),
            pending_invalidations: Mutex::new(HashSet::new()),
        }
    }
    pub fn with_startup_config(
        startup: plain_rs::host::startup::StartupConfig,
    ) -> Result<Self, String> {
        #[cfg(target_os = "ios")]
        let startup = {
            let mut config = startup;
            plain_rs::platform::startup::read_device_info(&mut config)?;
            config
        };
        startup.validate()?;
        Ok(Self::new(startup))
    }
    pub fn startup(&self) -> &plain_rs::host::startup::StartupConfig {
        &self.startup
    }
    pub fn uses_system_media_catalog(&self) -> bool {
        self.startup.system_media_catalog
    }
    pub fn needs_socket_cleanup(&self) -> bool {
        self.state.lock().unwrap().peer_sockets
    }
    pub fn connection_changes(&self) -> watch::Receiver<u64> {
        self.connection.subscribe()
    }
    pub fn generation(&self) -> Option<u64> {
        let state = self.state.lock().unwrap();
        state.sender.as_ref().map(|_| state.generation)
    }
    pub fn connected(&self) -> bool {
        self.state.lock().unwrap().sender.is_some()
    }
    pub fn connect(&self) -> (u64, mpsc::Receiver<Value>) {
        let (sender, receiver) = mpsc::channel(REQUEST_CAPACITY);
        let mut state = self.state.lock().unwrap();
        for (_, pending) in state.pending.drain() {
            let _ = pending.reply.send(Err("host connection replaced".into()));
        }
        state.generation += 1;
        state.sender = Some(sender);
        state.peer_sockets = false;
        let generation = state.generation;
        self.connection.send_replace(generation);
        drop(state);
        self.ready.notify_waiters();
        (generation, receiver)
    }
    pub fn is_current(&self, generation: u64) -> bool {
        let state = self.state.lock().unwrap();
        state.generation == generation && state.sender.is_some()
    }
    pub fn disconnect(&self, generation: u64) {
        let mut state = self.state.lock().unwrap();
        if state.generation != generation {
            return;
        }
        state.sender = None;
        self.connection.send_replace(generation);
        for (_, pending) in state.pending.drain() {
            let _ = pending.reply.send(Err("host disconnected".into()));
        }
    }
    pub fn reply(&self, generation: u64, message: Value) -> Result<(), String> {
        let id = message
            .get("id")
            .and_then(Value::as_u64)
            .ok_or("invalid host reply ID")?;
        let mut state = self.state.lock().unwrap();
        if state.generation != generation {
            return Ok(());
        }
        if let Some(pending) = state.pending.remove(&id) {
            let result = match message.get("error") {
                Some(Value::String(error)) => Err(error.clone()),
                Some(_) => Err("invalid host error".into()),
                None => message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| "missing host result".into()),
            };
            let _ = pending.reply.send(result);
        }
        Ok(())
    }
    pub async fn call(&self, method: &str, params: Value) -> Reply {
        self.call_inner(method, params, Some(Duration::from_secs(25)))
            .await
    }

    pub fn notify(&self, method: &'static str, params: Value) -> Result<(), String> {
        #[cfg(any(test, feature = "test-seams"))]
        self.callback_attempts.fetch_add(1, Ordering::Relaxed);
        let activity = if method == "systemWebActivity" {
            if self.activity_in_flight.swap(true, Ordering::AcqRel) {
                return Ok(());
            }
            Some(ActivityCall(self.activity_in_flight.clone()))
        } else {
            None
        };
        let permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| "host request capacity exceeded")?;
        self.notify_reserved(method, params, permit, activity)
    }

    pub fn invalidate(self: &Arc<Self>, method: &'static str) {
        let mut pending = self.pending_invalidations.lock().unwrap();
        if pending.contains(method) {
            return;
        }
        if self.notify(method, json!({})).is_ok() || !self.connected() {
            return;
        }
        pending.insert(method);
        let host = self.clone();
        tokio::spawn(async move {
            let permit = host.reserve_callback().await;
            let mut pending = host.pending_invalidations.lock().unwrap();
            if let Ok(permit) = permit {
                if let Err(error) = host.notify_reserved_callback(method, json!({}), permit) {
                    log::warn!("Host callback {method} recovery: {error}");
                }
            }
            pending.remove(method);
        });
    }

    pub async fn reserve_callback(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        self.capacity
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "host request capacity closed".into())
    }

    pub fn notify_reserved_callback(
        &self,
        method: &'static str,
        params: Value,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<(), String> {
        self.notify_reserved(method, params, permit, None)
    }

    fn notify_reserved(
        &self,
        method: &'static str,
        params: Value,
        permit: tokio::sync::OwnedSemaphorePermit,
        activity: Option<ActivityCall>,
    ) -> Result<(), String> {
        let (id, receiver) = {
            let mut state = self.state.lock().unwrap();
            let sender = state.sender.clone().ok_or("host unavailable")?;
            state.next_id += 1;
            let id = state.next_id;
            let (reply, receiver) = oneshot::channel();
            state.pending.insert(
                id,
                PendingReply {
                    reply,
                    _permit: permit,
                },
            );
            if sender
                .try_send(json!({"id":id,"method":method,"params":params}))
                .is_err()
            {
                state.pending.remove(&id);
                return Err("host request queue unavailable".into());
            }
            (id, receiver)
        };
        let pending = Pending {
            state: self.state.clone(),
            id,
        };
        tokio::spawn(async move {
            let _activity = activity;
            let _pending = pending;
            match tokio::time::timeout(Duration::from_secs(25), receiver).await {
                Ok(Ok(Ok(_))) => {}
                Ok(Ok(Err(error))) => log::warn!("Host callback {method} failed: {error}"),
                Ok(Err(_)) => log::warn!("Host callback {method} reply canceled"),
                Err(_) => log::warn!("Host callback {method} timed out"),
            }
        });
        Ok(())
    }
    pub async fn call_wait(&self, method: &str, params: Value) -> Reply {
        self.call_inner(method, params, None).await
    }
    pub async fn call_current(&self, generation: u64, method: &str, params: Value) -> Reply {
        self.request_inner(
            method,
            params,
            Some(Duration::from_secs(25)),
            Some(generation),
            None,
        )
        .await
    }
    pub async fn call_reserved_current(
        &self,
        generation: u64,
        method: &str,
        params: Value,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Reply {
        self.request_inner(
            method,
            params,
            Some(Duration::from_secs(25)),
            Some(generation),
            Some(permit),
        )
        .await
    }
    async fn call_inner(&self, method: &str, params: Value, timeout: Option<Duration>) -> Reply {
        if method == "systemProperties" {
            let names = params["names"]
                .as_array()
                .ok_or("System property names required")?;
            let mut values = serde_json::Map::new();
            let mut dynamic = Vec::new();
            for name in names {
                let name = name.as_str().ok_or("Invalid system property name")?;
                if !is_dynamic_property(name) {
                    return Err(format!("Unsupported dynamic system property: {name}"));
                }
                if !dynamic.iter().any(|existing| existing == name) {
                    dynamic.push(name.to_owned());
                }
            }
            if !dynamic.is_empty() {
                let mut params = params;
                params["names"] = json!(dynamic);
                let sampled = self.request(method, params, timeout).await?;
                let sampled = sampled
                    .as_object()
                    .ok_or("Invalid system property sample")?;
                for name in dynamic {
                    values.insert(
                        name.clone(),
                        sampled
                            .get(&name)
                            .ok_or_else(|| format!("Missing system property: {name}"))?
                            .clone(),
                    );
                }
            }
            return Ok(Value::Object(values));
        }
        self.request(method, params, timeout).await
    }
    async fn request(&self, method: &str, params: Value, timeout: Option<Duration>) -> Reply {
        self.request_inner(method, params, timeout, None, None)
            .await
    }
    async fn request_inner(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
        generation: Option<u64>,
        reserved: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Reply {
        let permit = match reserved {
            Some(permit) => permit,
            None => self
                .capacity
                .clone()
                .try_acquire_owned()
                .map_err(|_| "host request capacity exceeded")?,
        };
        let mut permit = Some(permit);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let (id, receiver) = loop {
            let notified = self.ready.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let request = {
                let mut state = self.state.lock().unwrap();
                if generation.is_some_and(|generation| {
                    state.generation != generation || state.sender.is_none()
                }) {
                    return Err("host connection replaced".into());
                }
                if let Some(sender) = state.sender.clone() {
                    if method == "peerTransportSocketOpen" {
                        state.peer_sockets = true;
                    }
                    state.next_id += 1;
                    let id = state.next_id;
                    let (reply, receiver) = oneshot::channel();
                    state.pending.insert(
                        id,
                        PendingReply {
                            reply,
                            _permit: permit.take().unwrap(),
                        },
                    );
                    if sender
                        .try_send(json!({"id":id,"method":method,"params":params}))
                        .is_err()
                    {
                        state.pending.remove(&id);
                        return Err("host request queue unavailable".into());
                    }
                    Some((id, receiver))
                } else {
                    None
                }
            };
            if let Some(request) = request {
                break request;
            }
            tokio::time::timeout_at(deadline, notified)
                .await
                .map_err(|_| "host unavailable")?;
        };
        let _pending = Pending {
            state: self.state.clone(),
            id,
        };
        match timeout {
            Some(timeout) => tokio::time::timeout(timeout, receiver)
                .await
                .map_err(|_| "host request timed out")?,
            None => receiver.await,
        }
        .map_err(|_| "host reply canceled")?
    }
}

#[cfg(test)]
#[path = "../../tests/unit/host/bridge.rs"]
mod tests;
