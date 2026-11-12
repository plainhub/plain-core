use crate::{http::websocket::runtime, managed_peers::PeerRoute, server::ServerState};
use futures_util::{SinkExt, StreamExt};
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::tokio_tungstenite::tungstenite::{Message, protocol::WebSocketConfig};
use plain_rs::infrastructure::{
    host::request_scope::NativeRequestScope,
    protocols::websocket::{self, Stream},
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};

pub const MAX_SOCKET_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SOCKET_CONTROL_BYTES: usize = 65536 - 48;
const QUEUE_CAPACITY: usize = 8;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SocketKind {
    Screen,
    ImageEditor,
}

impl SocketKind {
    fn channel(self) -> runtime::Channel {
        match self {
            Self::Screen => runtime::Channel::Screen,
            Self::ImageEditor => runtime::Channel::ImageEditor,
        }
    }
    fn path(self) -> &'static str {
        match self {
            Self::Screen => "/screen",
            Self::ImageEditor => "/image_editor",
        }
    }
    fn frame(self, event: &str, payload: &[u8]) -> Result<Vec<u8>> {
        let kind = match (self, event) {
            (Self::Screen, "SCREEN_MIRROR_VIDEO") => 1,
            (Self::Screen, "SCREEN_MIRROR_AUDIO") => 2,
            (Self::ImageEditor, "IMAGE_EDITOR_READY") if payload.is_empty() => 3,
            (Self::ImageEditor, "IMAGE_EDITOR_UPDATE") => 4,
            _ => return Err(anyhow!("Invalid socket event")),
        };
        ensure!(
            payload.len() < MAX_SOCKET_FRAME_BYTES,
            "Socket frame too large"
        );
        let mut bytes = Vec::with_capacity(payload.len() + 1);
        bytes.push(kind);
        bytes.extend_from_slice(payload);
        Ok(bytes)
    }
}

pub(crate) struct Sessions {
    entries: Mutex<HashMap<String, Arc<SocketSession>>>,
    bytes: Arc<tokio::sync::Semaphore>,
}
impl Default for Sessions {
    fn default() -> Self {
        Self {
            entries: Mutex::default(),
            bytes: Arc::new(tokio::sync::Semaphore::new(64 * 1024 * 1024)),
        }
    }
}
struct QueuedFrame {
    bytes: Vec<u8>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}
struct Output {
    frames: mpsc::Sender<QueuedFrame>,
    bytes: Arc<tokio::sync::Semaphore>,
}
struct Command {
    payload: Vec<u8>,
    result: oneshot::Sender<Result<(), String>>,
}
pub struct SocketSession {
    kind: SocketKind,
    scope: NativeRequestScope,
    closed: watch::Sender<bool>,
    commands: mpsc::Sender<Command>,
    frames: tokio::sync::Mutex<mpsc::Receiver<QueuedFrame>>,
    error: Mutex<Option<String>>,
}
impl SocketSession {
    pub fn scope_id(&self) -> &str {
        self.scope.id()
    }
    pub fn close(&self) {
        self.closed.send_replace(true);
    }
    async fn cancelled(&self) {
        let mut closed = self.closed.subscribe();
        tokio::select! { _ = self.scope.wait_closed() => {}, _ = closed.wait_for(|v| *v) => {} }
    }
    fn current(&self) -> Result<(), String> {
        if *self.closed.borrow() || self.scope.is_closed() {
            return Err(self
                .error
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "Socket session closed".into()));
        }
        Ok(())
    }
    pub async fn send(&self, payload: Vec<u8>) -> Result<(), String> {
        self.current()?;
        if self.kind != SocketKind::Screen {
            return Err("Socket is read-only".into());
        }
        if payload.is_empty() || payload.len() > MAX_SOCKET_CONTROL_BYTES {
            return Err("Invalid socket control length".into());
        }
        let (result, reply) = oneshot::channel();
        self.commands
            .try_send(Command { payload, result })
            .map_err(|_| "Socket command backpressure")?;
        tokio::select! {
            biased;
            _ = self.cancelled() => Err("Socket session closed".into()),
            result = reply => result.map_err(|_| "Socket session closed".to_owned())?,
        }
    }
    pub async fn read(&self) -> Result<Vec<u8>, String> {
        self.current()?;
        let mut frames = self
            .frames
            .try_lock()
            .map_err(|_| "Concurrent socket reader")?;
        let bytes = tokio::select! {
            biased;
            _ = self.cancelled() => return Err(self.error.lock().unwrap().clone().unwrap_or_else(|| "Socket session closed".into())),
            frame = frames.recv() => frame.ok_or("Socket session closed")?,
        };
        self.current()?;
        Ok(bytes.bytes)
    }
}

#[derive(Clone)]
pub struct NativeSockets {
    state: ServerState,
}
impl NativeSockets {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }
    pub fn open(
        &self,
        kind: SocketKind,
        peer_id: Option<String>,
        scope: NativeRequestScope,
    ) -> Result<String, String> {
        if scope.is_closed() || *self.state.stop.borrow() {
            return Err("Socket session closed".into());
        }
        let id = plain_libs::utils::uuid::Uuid::new_v4().into_string();
        let (commands, incoming) = mpsc::channel(QUEUE_CAPACITY);
        let (outgoing, frames) = mpsc::channel(QUEUE_CAPACITY);
        let outgoing = Output {
            frames: outgoing,
            bytes: self.state.main_ws.native_sockets.bytes.clone(),
        };
        let (closed, _) = watch::channel(false);
        let session = Arc::new(SocketSession {
            kind,
            scope,
            closed,
            commands,
            frames: tokio::sync::Mutex::new(frames),
            error: Mutex::default(),
        });
        {
            let mut entries = self.state.main_ws.native_sockets.entries.lock().unwrap();
            if entries.len() >= 128 {
                return Err("Socket capacity reached".into());
            }
            entries.insert(id.clone(), session.clone());
        }
        let state = self.state.clone();
        let handle = id.clone();
        tokio::spawn(async move {
            let mut stop = state.stop.clone();
            let result = tokio::select! {
                biased;
                _ = session.cancelled() => Ok(()),
                _ = stop.wait_for(|v| *v) => Err(anyhow!("Server stopped")),
                result = async {
                    match peer_id {
                        Some(peer_id) => peer(&state, &session, &peer_id, incoming, &outgoing).await,
                        None => local(&state, &handle, &session, incoming, &outgoing).await,
                    }
                } => result,
            };
            if let Err(error) = result {
                *session.error.lock().unwrap() = Some(error.to_string());
            }
            session.close();
            {
                let mut frames = session.frames.lock().await;
                frames.close();
                while frames.try_recv().is_ok() {}
            }
            runtime::remove_connection(&state, &handle).await;
            state
                .main_ws
                .native_sockets
                .entries
                .lock()
                .unwrap()
                .remove(&handle);
        });
        Ok(id)
    }
    pub fn session(&self, handle: &str) -> Result<Arc<SocketSession>, String> {
        self.state
            .main_ws
            .native_sockets
            .entries
            .lock()
            .unwrap()
            .get(handle)
            .cloned()
            .ok_or_else(|| "Unknown socket session".into())
    }
}

fn publish(outgoing: &Output, frame: Vec<u8>) -> Result<()> {
    let permit = outgoing
        .bytes
        .clone()
        .try_acquire_many_owned(frame.len() as u32)
        .map_err(|_| anyhow!("Socket byte backpressure"))?;
    outgoing
        .frames
        .try_send(QueuedFrame {
            bytes: frame,
            _permit: permit,
        })
        .map_err(|_| anyhow!("Socket client backpressure"))
}

async fn local(
    state: &ServerState,
    id: &str,
    session: &SocketSession,
    mut commands: mpsc::Receiver<Command>,
    outgoing: &Output,
) -> Result<()> {
    let client = state
        .native_client_id()
        .map_err(plain_libs::anyhow::Error::msg)?;
    if session.kind == SocketKind::Screen {
        plain_rs::features::services::capabilities::require(&state.host, "SCREEN_MIRROR").await?;
    }
    let channel = session.kind.channel();
    let mut events = channel.subscribe(state);
    let mut closed = state.main_ws.register_native(id, client.clone(), channel);
    publish(outgoing, vec![0])?;
    if session.kind == SocketKind::ImageEditor {
        publish(outgoing, session.kind.frame("IMAGE_EDITOR_READY", &[])?)?;
    }
    loop {
        tokio::select! {
            biased;
            _ = closed.changed() => return Err(anyhow!("Socket session revoked")),
            command = commands.recv() => {
                let Some(command) = command else { return Ok(()); };
                let result = runtime::dispatch_controls(state, &command.payload).await;
                let failed = result.is_err();
                let _ = command.result.send(result);
                if failed { return Err(anyhow!("Screen controls unavailable")); }
            },
            event = events.recv() => {
                let event = event.map_err(|error| anyhow!("Socket event stream: {error}"))?;
                let Some(event) = channel.public_event(event, &client) else { continue; };
                let payload = event.binary_payload.unwrap_or_else(|| event.payload.into_bytes());
                publish(outgoing, session.kind.frame(event.event_type, &payload)?)?;
            }
        }
    }
}

async fn peer(
    state: &ServerState,
    session: &SocketSession,
    peer_id: &str,
    mut commands: mpsc::Receiver<Command>,
    outgoing: &Output,
) -> Result<()> {
    let (route, mut socket, mut stream) = tokio::time::timeout(Duration::from_secs(10), async {
        let route = state.managed_proxy.peers.route(state, peer_id).await?;
        let mut url = route.base.join(session.kind.path())?;
        url.set_scheme("wss")
            .map_err(|_| anyhow!("Invalid managed address"))?;
        url.query_pairs_mut().append_pair("cid", &route.client_id);
        let (mut socket, _) = plain_libs::tokio_tungstenite::connect_async_tls_with_config(
            url.as_str(),
            Some(
                WebSocketConfig::default()
                    .max_message_size(Some(MAX_SOCKET_FRAME_BYTES + 128))
                    .max_frame_size(Some(MAX_SOCKET_FRAME_BYTES + 128)),
            ),
            false,
            Some(plain_libs::tokio_tungstenite::Connector::Rustls(Arc::new(
                plain_libs::protocols::http_client::pinned_tls_config(route.pin)?,
            ))),
        )
        .await?;
        route.check_current(state)?;
        let path = session.kind.path();
        let registration = websocket::registration(&route.key, &route.client_id, path)
            .ok_or_else(|| anyhow!("Socket registration failed"))?;
        socket
            .send(Message::Binary(registration.clone().into()))
            .await?;
        let Message::Binary(ack) = socket
            .next()
            .await
            .ok_or_else(|| anyhow!("Socket handshake closed"))??
        else {
            return Err(anyhow!("Encrypted socket acknowledgement required"));
        };
        let mut stream =
            websocket::acknowledge(&route.key, &route.client_id, path, &registration, &ack)
                .ok_or_else(|| anyhow!("Invalid socket acknowledgement"))?;
        let confirmation = stream
            .encode(&[], &route.key)
            .ok_or_else(|| anyhow!("Socket confirmation failed"))?;
        socket.send(Message::Binary(confirmation.into())).await?;
        route.check_current(state)?;
        Ok::<_, plain_libs::anyhow::Error>((route, socket, stream))
    })
    .await??;
    publish(outgoing, vec![0])?;
    let mut changed = state.db.peer_changes();
    let result = async {
        loop {
            route.check_current(state)?;
            tokio::select! {
                biased;
                _ = changed.changed() => { route.check_current(state)?; },
                command = commands.recv() => {
                    let Some(command) = command else { return Ok(()); };
                    let result = send_peer(&route, &mut socket, &mut stream, &command.payload).await;
                    let failed = result.is_err();
                    let _ = command.result.send(result.map_err(|error| error.to_string()));
                    if failed { return Err(anyhow!("Socket send failed")); }
                },
                message = socket.next() => match message {
                    Some(Ok(Message::Binary(bytes))) => {
                        route.check_current(state)?;
                        let plain = stream.decode(&bytes, &route.key).ok_or_else(|| anyhow!("Invalid socket frame"))?;
                        let (kind, payload) = websocket::decode_raw(&plain).ok_or_else(|| anyhow!("Invalid socket event"))?;
                        publish(outgoing, session.kind.frame(kind, payload)?)?;
                    },
                    Some(Ok(Message::Ping(bytes))) => { socket.send(Message::Pong(bytes)).await?; },
                    Some(Ok(Message::Pong(_))) => {},
                    _ => return Err(anyhow!("Socket connection closed")),
                }
            }
        }
    }.await;
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close(None)).await;
    result
}

async fn send_peer(
    route: &PeerRoute,
    socket: &mut plain_libs::tokio_tungstenite::WebSocketStream<
        plain_libs::tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    stream: &mut Stream,
    payload: &[u8],
) -> Result<()> {
    let encrypted = stream
        .encode(payload, &route.key)
        .ok_or_else(|| anyhow!("Socket control encryption failed"))?;
    tokio::time::timeout(
        Duration::from_secs(2),
        socket.send(Message::Binary(encrypted.into())),
    )
    .await??;
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/native/sockets.rs"]
mod tests;
