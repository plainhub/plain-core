use super::managed_resources::recovery;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::chat_store::peers;
use crate::infrastructure::host::request_scope::NativeRequestScope;
use crate::infrastructure::protocols::graphql::PeerSubscriptionTarget;
use crate::infrastructure::protocols::graphql::messages::ClientMessage;
use crate::infrastructure::protocols::graphql::messages::ServerMessage;
use crate::infrastructure::protocols::websocket;
use crate::runtime::Runtime as CoreRuntime;
use futures_util::{SinkExt, StreamExt};
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::protocols::http_client;
use plain_libs::tokio_tungstenite::tungstenite::Message;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{mpsc, watch};

pub struct PeerSubscription {
    state: CoreRuntime,
    peer: DPeer,
    client: String,
    scope: NativeRequestScope,
    pin: Option<[u8; 32]>,
    closed: watch::Sender<bool>,
    commands: mpsc::Sender<ClientMessage>,
    messages: tokio::sync::Mutex<mpsc::Receiver<Result<String, String>>>,
}
impl PeerSubscription {
    pub fn open(
        state: CoreRuntime,
        target: PeerSubscriptionTarget,
        scope: NativeRequestScope,
    ) -> Result<Arc<Self>, String> {
        Self::open_with_pin(state, target, scope, None)
    }
    pub fn open_with_pin(
        state: CoreRuntime,
        target: PeerSubscriptionTarget,
        scope: NativeRequestScope,
        pin: Option<[u8; 32]>,
    ) -> Result<Arc<Self>, String> {
        if scope.is_closed() || *state.stop.borrow() {
            return Err("Subscription session closed".into());
        }
        let peer = peers::get(&state.db, &target.peer_id)
            .map_err(|error| error.to_string())?
            .ok_or("Peer removed")?;
        if !peer.is_paired()
            || target.public_key.is_empty()
            || peer.public_key != target.public_key
            || plain_libs::utils::base64::base64_decode(&peer.token).len() != 32
        {
            return Err("Management session unavailable".into());
        }
        let client = state.native_client_id()?;
        if peer.id == client {
            return Err("Remote target must differ from local device".into());
        }
        let (commands, incoming) = mpsc::channel(64);
        let (outgoing, messages) = mpsc::channel(64);
        let (closed, _) = watch::channel(false);
        let session = Arc::new(Self {
            state,
            peer,
            client,
            scope,
            pin,
            closed,
            commands,
            messages: tokio::sync::Mutex::new(messages),
        });
        let running = session.clone();
        tokio::spawn(async move {
            let mut closed = running.closed.subscribe();
            let mut stop = running.state.stop.clone();
            let result = tokio::select! {
                biased;
                _ = closed.wait_for(|value| *value) => return,
                _ = stop.wait_for(|value| *value) => Err(anyhow!("Server stopped")),
                _ = running.scope.wait_closed() => Err(anyhow!("Subscription scope closed")),
                result = running.run(incoming, &outgoing) => result,
            };
            if let Err(error) = result {
                let _ = outgoing.try_send(Err(error.to_string()));
            }
            running.close();
        });
        Ok(session)
    }
    pub fn scope_id(&self) -> &str {
        self.scope.id()
    }
    pub fn client_id(&self) -> &str {
        &self.client
    }
    pub fn close(&self) {
        self.closed.send_replace(true);
    }
    pub async fn wait_closed(&self) {
        let mut closed = self.closed.subscribe();
        let mut stop = self.state.stop.clone();
        tokio::select! {
            _ = closed.wait_for(|value| *value) => {},
            _ = stop.wait_for(|value| *value) => {},
            _ = self.scope.wait_closed() => {},
        }
    }
    fn current(&self) -> Result<()> {
        ensure!(
            !*self.closed.borrow() && !self.scope.is_closed(),
            "Subscription session closed"
        );
        let current =
            peers::get(&self.state.db, &self.peer.id)?.ok_or_else(|| anyhow!("Peer removed"))?;
        ensure!(
            current.is_paired()
                && current.public_key == self.peer.public_key
                && current.token == self.peer.token,
            "Management session revoked"
        );
        Ok(())
    }
    pub fn send(&self, encoded: &str) -> Result<(), String> {
        self.current().map_err(|error| error.to_string())?;
        if encoded.len() > 1024 * 1024 - 48 {
            return Err("Subscription request too large".into());
        }
        let message =
            plain_libs::serde_json::from_str(encoded).map_err(|error| error.to_string())?;
        self.commands
            .try_send(message)
            .map_err(|_| "Subscription command backpressure".into())
    }
    pub async fn read(&self) -> Result<String, String> {
        let mut messages = self
            .messages
            .try_lock()
            .map_err(|_| "Concurrent subscription reader")?;
        let mut closed = self.closed.subscribe();
        let result = tokio::select! {
            biased;
            _ = closed.wait_for(|value| *value) => return Err("Subscription session closed".into()),
            message = messages.recv() => message.ok_or("Subscription connection closed")?,
        };
        self.current().map_err(|error| error.to_string())?;
        result
    }
    async fn run(
        &self,
        mut commands: mpsc::Receiver<ClientMessage>,
        outgoing: &mpsc::Sender<Result<String, String>>,
    ) -> Result<()> {
        let mut peer = self.peer.clone();
        let mut pin = self.pin;
        let mut operations = HashMap::new();
        loop {
            let result = self
                .exchange(&peer, pin, &mut commands, outgoing, &mut operations)
                .await;
            let Err(error) = result else { return result };
            let latest = recovery::current(&self.state, &self.peer)?;
            if recovery::tls_rejection(&error)
                || (!recovery::moved(&latest, &peer) && !transport_interrupted(&error))
            {
                return Err(error);
            }
            let mut outcome = self
                .state
                .managed_resources
                .recovery
                .request(&self.state, &peer)?;
            let route = recovery::wait(
                &mut outcome,
                tokio::time::Instant::now() + recovery::RECOVERY_TIMEOUT,
            )
            .await?;
            peer = route.peer.clone();
            pin = Some(route.pin);
        }
    }

    fn check_address(&self, peer: &DPeer) -> Result<()> {
        self.current()?;
        let latest = recovery::current(&self.state, peer)?;
        ensure!(
            !recovery::moved(&latest, peer),
            "Management address changed"
        );
        Ok(())
    }

    async fn exchange(
        &self,
        peer: &DPeer,
        pin: Option<[u8; 32]>,
        commands: &mut mpsc::Receiver<ClientMessage>,
        outgoing: &mpsc::Sender<Result<String, String>>,
        operations: &mut HashMap<String, ClientMessage>,
    ) -> Result<()> {
        use crate::features::services::peers::management_wire;
        self.check_address(peer)?;
        let mut changed = self.state.db.peer_changes();
        let key = plain_libs::utils::base64::base64_decode(&peer.token);
        let mut url =
            http_client::Url::parse(&peer.base_url().ok_or_else(|| anyhow!("No peer address"))?)?;
        url.set_scheme("wss")
            .map_err(|_| anyhow!("Invalid management address"))?;
        url.query_pairs_mut().append_pair("cid", &self.client);
        let mut socket = tokio::time::timeout(
            Duration::from_secs(10),
            crate::features::services::peers::status_socket::connect_management_with_pin(
                url.as_str(),
                pin,
            ),
        )
        .await??;
        let init = websocket::registration(&key, &self.client, "/graphql")
            .ok_or_else(|| anyhow!("Invalid management session key"))?;
        socket.send(Message::Binary(init.clone().into())).await?;
        let ack = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await?
            .ok_or_else(|| anyhow!("Management handshake closed"))??;
        let Message::Binary(ack) = ack else {
            return Err(anyhow!("Encrypted management acknowledgement required"));
        };
        let mut stream = websocket::acknowledge(&key, &self.client, "/graphql", &init, &ack)
            .ok_or_else(|| anyhow!("Invalid management acknowledgement"))?;
        self.check_address(peer)?;
        let confirmation = stream
            .encode(&[], &key)
            .ok_or_else(|| anyhow!("Invalid management confirmation"))?;
        socket.send(Message::Binary(confirmation.into())).await?;
        for operation in operations.values() {
            socket
                .send(Message::Binary(
                    management_wire::encode(operation, &key, &mut stream)?.into(),
                ))
                .await?;
        }
        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(20),
            Duration::from_secs(20),
        );
        let mut pong_deadline = None;
        loop {
            self.check_address(peer)?;
            tokio::select! {
                result = changed.changed() => { result?; self.check_address(peer)?; },
                _ = heartbeat.tick() => {
                    socket.send(Message::Binary(crate::features::services::peers::management_wire::encode(&ClientMessage::Ping { payload: None }, &key, &mut stream)?.into())).await?;
                    pong_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(10));
                },
                _ = async { if let Some(deadline) = pong_deadline { tokio::time::sleep_until(deadline).await } else { std::future::pending::<()>().await } } => return Err(plain_libs::tokio_tungstenite::tungstenite::Error::ConnectionClosed.into()),
                command = commands.recv() => {
                    let command = command.ok_or_else(|| anyhow!("Subscription session closed"))?;
                    self.current()?;
                    let encoded = management_wire::encode(&command, &key, &mut stream)?;
                    match &command {
                        ClientMessage::Subscribe { id, .. } => {
                            ensure!(operations.len() < 64 || operations.contains_key(id), "Too many active subscriptions");
                            operations.insert(id.clone(), command);
                        }
                        ClientMessage::Complete { id } => { operations.remove(id); }
                        _ => {}
                    }
                    self.check_address(peer)?;
                    socket.send(Message::Binary(encoded.into())).await?;
                },
                frame = socket.next() => {
                    let frame = frame.ok_or(plain_libs::tokio_tungstenite::tungstenite::Error::ConnectionClosed)??;
                    match frame {
                        Message::Binary(frame) => {
                            let plain = stream.decode(&frame, &key).ok_or_else(|| anyhow!("Invalid management event authentication"))?;
                            let message: ServerMessage = plain_libs::serde_json::from_slice(&plain)?;
                            self.check_address(peer)?;
                            if let ServerMessage::Complete { id } | ServerMessage::Error { id, .. } = &message {
                                operations.remove(id);
                            }
                            if matches!(message, ServerMessage::Pong { .. }) { pong_deadline = None; }
                            else { outgoing.try_send(Ok(plain_libs::serde_json::to_string(&message)?)).map_err(|_| anyhow!("Subscription response backpressure"))?; }
                        },
                        Message::Ping(payload) => socket.send(Message::Pong(payload)).await?,
                        Message::Pong(_) => {},
                        _ => return Err(anyhow!("Encrypted management frame required")),
                    }
                },
            }
        }
    }
}

fn transport_interrupted(error: &plain_libs::anyhow::Error) -> bool {
    use plain_libs::tokio_tungstenite::tungstenite::Error;
    error.is::<tokio::time::error::Elapsed>()
        || matches!(
            error.downcast_ref::<Error>(),
            Some(Error::ConnectionClosed | Error::AlreadyClosed)
        )
        || matches!(
            error.downcast_ref::<Error>(),
            Some(Error::Io(io)) if matches!(
                io.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::NotConnected | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::NetworkUnreachable
                    | std::io::ErrorKind::HostUnreachable
            )
        )
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/subscriptions.rs"]
mod tests;
