use super::{
    events::authorize,
    operation::{self, ClientMessage, ServerMessage},
    registry::EventSpec,
};
use crate::{
    assembly::server::ServerState,
    graphql::executor::{RequestContext, RequestOrigin},
    native::graphql::NativeRequestScope,
};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use subtle::ConstantTimeEq;
use tokio::sync::{mpsc, watch};

pub(super) struct SubscriptionReady(pub watch::Sender<bool>);
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Default)]
pub struct Sessions {
    entries: Mutex<HashMap<u64, SubscriptionSession>>,
    host_delivery: Mutex<bool>,
}
struct Operation {
    scope: NativeRequestScope,
    task: Option<tokio::task::JoinHandle<()>>,
}
struct Queued {
    message: String,
    id: Option<String>,
    terminal: bool,
    spec: Option<&'static EventSpec>,
    scope: Option<NativeRequestScope>,
}
pub struct Session {
    pub id: u64,
    state: ServerState,
    context: RequestContext,
    origin: RequestOrigin,
    client: String,
    owner_scope: String,
    key: Option<Vec<u8>>,
    closed: watch::Sender<bool>,
    online: Mutex<bool>,
    outgoing: mpsc::Sender<Queued>,
    incoming: tokio::sync::Mutex<mpsc::Receiver<Queued>>,
    operations: Mutex<HashMap<String, Operation>>,
}
fn next_id() -> Result<u64, String> {
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value.checked_add(1).filter(|next| *next <= i64::MAX as u64)
    })
    .map_err(|_| "Subscription handle exhausted".into())
}

#[derive(Clone)]
pub enum SubscriptionSession {
    Local(Arc<Session>),
    Peer(Arc<crate::services::peers::subscriptions::PeerSubscription>),
}
impl SubscriptionSession {
    pub fn send(&self, message: &str) -> Result<(), String> {
        match self {
            Self::Local(session) => session.send(message),
            Self::Peer(session) => session.send(message),
        }
    }
    pub async fn read(&self) -> Result<String, String> {
        match self {
            Self::Local(session) => session.read().await,
            Self::Peer(session) => session.read().await,
        }
    }
    pub fn close(&self) {
        match self {
            Self::Local(session) => session.close(),
            Self::Peer(session) => session.close(),
        }
    }
    pub fn scope_id(&self) -> &str {
        match self {
            Self::Local(session) => session.scope_id(),
            Self::Peer(session) => session.scope_id(),
        }
    }
    fn client_id(&self) -> &str {
        match self {
            Self::Local(session) => session.client_id(),
            Self::Peer(session) => session.client_id(),
        }
    }
}
impl Sessions {
    pub(crate) fn open(
        self: &Arc<Self>,
        state: ServerState,
        context: RequestContext,
        client: String,
        key: Option<Vec<u8>>,
        scope: NativeRequestScope,
    ) -> Result<Arc<Session>, String> {
        let id = next_id()?;
        let (outgoing, incoming) = mpsc::channel(64);
        let (closed, _) = watch::channel(false);
        let origin = context.origin();
        let session = Arc::new(Session {
            id,
            state: state.clone(),
            context,
            origin,
            client,
            owner_scope: scope.id().to_owned(),
            key,
            closed,
            online: Mutex::new(false),
            outgoing,
            incoming: tokio::sync::Mutex::new(incoming),
            operations: Mutex::default(),
        });
        {
            let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
            if entries.len() >= 128 {
                return Err("Subscription session capacity reached".into());
            }
            entries.insert(id, SubscriptionSession::Local(session.clone()));
        }
        let sessions = Arc::downgrade(self);
        let mut stop = state.stop.clone();
        let mut closed = session.closed.subscribe();
        tokio::spawn(async move {
            tokio::select! { _ = stop.wait_for(|value| *value) => {}, _ = scope.wait_closed() => {}, _ = closed.wait_for(|value| *value) => {} }
            if let Some(sessions) = sessions.upgrade() {
                sessions.close(id);
            }
        });
        Ok(session)
    }
    pub(crate) fn open_peer(
        self: &Arc<Self>,
        state: ServerState,
        target: crate::native::graphql::PeerSubscriptionTarget,
        scope: NativeRequestScope,
    ) -> Result<u64, String> {
        let id = next_id()?;
        let session =
            crate::services::peers::subscriptions::PeerSubscription::open(state, target, scope)?;
        let mut entries = self.entries.lock().map_err(|error| error.to_string())?;
        if entries.len() >= 128 {
            session.close();
            return Err("Subscription session capacity reached".into());
        }
        entries.insert(id, SubscriptionSession::Peer(session.clone()));
        let sessions = Arc::downgrade(self);
        tokio::spawn(async move {
            session.wait_closed().await;
            if let Some(sessions) = sessions.upgrade() {
                sessions.close(id);
            }
        });
        Ok(id)
    }
    pub fn get(&self, id: u64) -> Result<SubscriptionSession, String> {
        self.entries
            .lock()
            .map_err(|e| e.to_string())?
            .get(&id)
            .cloned()
            .ok_or_else(|| "Unknown subscription session".into())
    }
    pub(crate) fn snapshot(&self) -> Vec<String> {
        let entries = self.entries.lock().unwrap();
        let mut clients: Vec<_> = entries
            .values()
            .filter_map(|session| match session {
                SubscriptionSession::Local(session) if *session.online.lock().unwrap() => Some(session.client.clone()),
                _ => None,
            })
            .collect();
        clients.sort();
        clients.dedup();
        clients
    }
    pub(crate) fn notify_snapshot(&self, host: &crate::host::bridge::Host) -> Result<(), String> {
        let _delivery = self.host_delivery.lock().unwrap();
        host.notify("onlineClientsChanged", serde_json::json!({"clientIds":self.snapshot()}))
    }

    fn notify(state: &ServerState) {
        let sessions = &state.subscription_sessions;
        let mut recovering = sessions.host_delivery.lock().unwrap();
        if let Err(error) = state.host.notify("onlineClientsChanged", serde_json::json!({"clientIds":sessions.snapshot()})) {
            log::warn!("Online clients callback: {error}");
            if *recovering || !state.host.connected() { return; }
            *recovering = true;
            let state = state.clone();
            tokio::spawn(async move {
                let permit = state.host.reserve_callback().await;
                let sessions = &state.subscription_sessions;
                let mut recovering = sessions.host_delivery.lock().unwrap();
                if let Ok(permit) = permit {
                    if let Err(error) = state.host.notify_reserved_callback(
                        "onlineClientsChanged", serde_json::json!({"clientIds":sessions.snapshot()}), permit,
                    ) {
                        log::warn!("Online clients snapshot callback: {error}");
                    }
                }
                *recovering = false;
            });
        }
    }
    pub fn close(&self, id: u64) {
        let session = self.entries.lock().unwrap().remove(&id);
        if let Some(session) = session {
            session.close();
        }
    }
    pub(crate) fn close_client(&self, client: Option<&str>) {
        let sessions = {
            let mut entries = self.entries.lock().unwrap();
            let ids: Vec<_> = entries
                .iter()
                .filter(|(_, session)| client.is_none_or(|id| id == session.client_id()))
                .map(|(id, _)| *id)
                .collect();
            ids.into_iter()
                .filter_map(|id| entries.remove(&id))
                .collect::<Vec<_>>()
        };
        for session in sessions {
            session.close();
        }
    }
}
impl Session {
    pub(crate) fn client_id(&self) -> &str {
        &self.client
    }
    pub fn scope_id(&self) -> &str {
        &self.owner_scope
    }
    pub(crate) fn mark_online(&self) -> Result<(), String> {
        let mut online = self.online.lock().unwrap();
        if *self.closed.borrow() {
            return Err("Subscription session closed".into());
        }
        *online = true;
        drop(online);
        Sessions::notify(&self.state);
        Ok(())
    }
    pub fn close(&self) {
        let mut online = self.online.lock().unwrap();
        self.closed.send_replace(true);
        let changed = std::mem::replace(&mut *online, false);
        drop(online);
        for (_, operation) in self.operations.lock().unwrap().drain() {
            operation.scope.close();
            if let Some(task) = operation.task {
                task.abort();
            }
        }
        if changed {
            Sessions::notify(&self.state);
        }
    }
    fn valid(&self) -> bool {
        if *self.closed.borrow() || *self.state.stop.borrow() {
            return false;
        }
        if self.origin == RequestOrigin::Local && self.key.is_none() {
            return self.state.native_client_id().ok().as_deref() == Some(self.client.as_str());
        }
        if !self.state.prefs.get_or("desktop_access", true) {
            return false;
        }
        let current =
            crate::services::sessions::local_key(&self.state, &self.client).or_else(|| {
                crate::services::sessions::key(&self.state.db, &self.client)
                    .ok()
                    .flatten()
            });
        match (&self.key, current) {
            (Some(expected), Some(current)) => {
                bool::from(expected.as_slice().ct_eq(current.as_slice()))
            }
            _ => false,
        }
    }
    fn queue(
        &self,
        message: ServerMessage,
        spec: Option<&'static EventSpec>,
        scope: Option<NativeRequestScope>,
    ) -> Result<(), String> {
        let (id, mut terminal) = match &message {
            ServerMessage::Next { id, .. } => (Some(id.clone()), false),
            ServerMessage::Error { id, .. } | ServerMessage::Complete { id } => {
                (Some(id.clone()), true)
            }
            ServerMessage::Pong { .. } => (None, false),
        };
        let mut encoded = serde_json::to_string(&message).map_err(|error| error.to_string())?;
        if encoded.len() > 1024 * 1024 - 49 {
            let Some(id) = &id else {
                self.close();
                return Err("Subscription response too large".into());
            };
            terminal = true;
            encoded = serde_json::to_string(&ServerMessage::Error {
                id: id.clone(),
                payload: vec![async_graphql::ServerError::new(
                    "subscription_result_too_large",
                    None,
                )],
            })
            .map_err(|error| error.to_string())?;
        }
        self.outgoing
            .try_send(Queued {
                message: encoded,
                id,
                terminal,
                spec,
                scope,
            })
            .map_err(|_| {
                self.closed.send_replace(true);
                "Subscription backpressure".to_owned()
            })
    }

    pub fn send(self: &Arc<Self>, encoded: &str) -> Result<(), String> {
        if !self.valid() {
            return Err("Subscription session closed".into());
        }
        if encoded.len() > 1024 * 1024 - 49 {
            return Err("Subscription request too large".into());
        }
        match serde_json::from_str::<ClientMessage>(encoded).map_err(|e| e.to_string())? {
            ClientMessage::Ping { payload } => {
                self.queue(ServerMessage::Pong { payload }, None, None)
            }
            ClientMessage::Pong { .. } => Ok(()),
            ClientMessage::Complete { id } => {
                if let Some(operation) = self.operations.lock().unwrap().remove(&id) {
                    operation.scope.close();
                    if let Some(task) = operation.task {
                        task.abort();
                    }
                }
                Ok(())
            }
            ClientMessage::Subscribe { id, payload } => {
                if id.is_empty() || id.len() > 128 {
                    return Err("Invalid subscription ID".into());
                }
                let mut operations = self.operations.lock().map_err(|error| error.to_string())?;
                if operations.contains_key(&id) {
                    return Err("Duplicate subscription ID".into());
                }
                if operations.len() >= 32 {
                    return Err("Subscription operation capacity reached".into());
                }
                let spec = match operation::spec(&payload) {
                    Ok(spec) => spec,
                    Err(error) => {
                        let scope = NativeRequestScope::default();
                        operations.insert(
                            id.clone(),
                            Operation {
                                scope: scope.clone(),
                                task: None,
                            },
                        );
                        return self.queue(
                            ServerMessage::Error {
                                id,
                                payload: vec![async_graphql::ServerError::new(error, None)],
                            },
                            None,
                            Some(scope),
                        );
                    }
                };
                let scope = NativeRequestScope::default();
                let task_scope = scope.clone();
                let (ready, mut readiness) = watch::channel(false);
                let mut ready_sent = false;
                let session = self.clone();
                let task_id = id.clone();
                let task = tokio::spawn(async move {
                    let mut stream = crate::graphql::executor::subscription_stream(
                        session.state.clone(),
                        payload
                            .data(task_scope.clone())
                            .data(SubscriptionReady(ready)),
                        session.context.clone(),
                    );
                    loop {
                        let response = tokio::select! {
                            biased;
                            _ = task_scope.wait_closed() => break,
                            changed = readiness.changed(), if !ready_sent => {
                                if changed.is_err() { ready_sent = true; }
                                else if *readiness.borrow() {
                                    ready_sent = true;
                                    let response = async_graphql::Response::default().extension("subscriptionReady", async_graphql::Value::Boolean(true));
                                    if session.queue(ServerMessage::Next { id: task_id.clone(), payload: response }, Some(spec), Some(task_scope.clone())).is_err() { break; }
                                }
                                continue;
                            },
                            response = stream.next() => response,
                        };
                        let Some(response) = response else {
                            break;
                        };
                        let terminal = response.data == async_graphql::Value::Null
                            && !response.errors.is_empty();
                        let message = if terminal {
                            ServerMessage::Error {
                                id: task_id.clone(),
                                payload: response.errors,
                            }
                        } else {
                            ServerMessage::Next {
                                id: task_id.clone(),
                                payload: response,
                            }
                        };
                        if session
                            .queue(message, Some(spec), Some(task_scope.clone()))
                            .is_err()
                        {
                            break;
                        }
                        if terminal {
                            return;
                        }
                    }
                    let _ = session.queue(
                        ServerMessage::Complete {
                            id: task_id.clone(),
                        },
                        Some(spec),
                        Some(task_scope),
                    );
                });
                operations.insert(
                    id,
                    Operation {
                        scope,
                        task: Some(task),
                    },
                );
                Ok(())
            }
        }
    }
    pub async fn read(&self) -> Result<String, String> {
        let mut receiver = self
            .incoming
            .try_lock()
            .map_err(|_| "Concurrent subscription reader")?;
        let mut closed = self.closed.subscribe();
        loop {
            let queued = tokio::select! {
                biased;
                _ = closed.wait_for(|value| *value) => return Err("Subscription session closed".into()),
                message = receiver.recv() => message.ok_or("Subscription session closed")?,
            };
            if !self.valid() {
                self.close();
                return Err("Subscription session revoked".into());
            }
            if queued
                .scope
                .as_ref()
                .is_some_and(NativeRequestScope::is_closed)
            {
                continue;
            }
            if let Some(spec) = queued.spec {
                if let Err(error) = authorize(&self.state, self.origin, spec).await {
                    if let Some(id) = &queued.id {
                        let id = id.clone();
                        if let Some(operation) = self.operations.lock().unwrap().remove(&id) {
                            operation.scope.close();
                            if let Some(task) = operation.task {
                                task.abort();
                            }
                        }
                        return serde_json::to_string(&ServerMessage::Error {
                            id,
                            payload: vec![async_graphql::ServerError::new(error.message, None)],
                        })
                        .map_err(|e| e.to_string());
                    }
                }
            }
            if !self.valid() {
                self.close();
                return Err("Subscription session revoked".into());
            }
            if queued.terminal {
                if let Some(operation) = queued
                    .id
                    .as_ref()
                    .and_then(|id| self.operations.lock().unwrap().remove(id))
                {
                    operation.scope.close();
                    if let Some(task) = operation.task {
                        task.abort();
                    }
                }
            }
            return Ok(queued.message);
        }
    }
}
