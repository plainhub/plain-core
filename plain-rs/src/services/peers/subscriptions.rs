use anyhow::{Result, anyhow, ensure};
use futures_util::{SinkExt, StreamExt};
use plain_rs::db::{DPeer, chat_store::peers};
use plain_rs::host::request_scope::NativeRequestScope;
use plain_rs::protocols::graphql::PeerSubscriptionTarget;
use plain_rs::protocols::graphql::messages::ClientMessage;
use plain_rs::protocols::graphql::messages::ServerMessage;
use plain_rs::runtime::Runtime as CoreRuntime;
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

pub struct PeerSubscription {
    state: CoreRuntime,
    peer: DPeer,
    client: String,
    scope: NativeRequestScope,
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
        let peer = peers::get(&state.db, &target.peer_id)
            .map_err(|error| error.to_string())?
            .ok_or("Peer removed")?;
        if !peer.is_paired()
            || target.public_key.is_empty()
            || peer.public_key != target.public_key
            || plain_rs::utils::base64::base64_decode(&peer.token).len() != 32
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
                result = running.exchange(incoming, &outgoing) => result,
            };
            if let Err(error) = result {
                let _ = outgoing.try_send(Err(error.to_string()));
            }
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
        if encoded.len() > 1024 * 1024 - 49 {
            return Err("Subscription request too large".into());
        }
        let message = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
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
    async fn exchange(
        &self,
        mut commands: mpsc::Receiver<ClientMessage>,
        outgoing: &mpsc::Sender<Result<String, String>>,
    ) -> Result<()> {
        self.current()?;
        let mut changed = self.state.db.peer_changes();
        let key = plain_rs::utils::base64::base64_decode(&self.peer.token);
        let mut url = reqwest::Url::parse(
            &plain_rs::services::peers::peer_address::address(&self.peer).base_url,
        )?;
        url.set_scheme("wss")
            .map_err(|_| anyhow!("Invalid management address"))?;
        url.query_pairs_mut().append_pair("cid", &self.client);
        let mut socket = tokio::time::timeout(
            Duration::from_secs(10),
            plain_rs::services::peers::status_socket::connect_management(url.as_str()),
        )
        .await??;
        let init = plain_rs::services::peers::management_wire::registration(
            &key,
            &self.client,
            chrono::Utc::now().timestamp_millis(),
        )?;
        socket.send(Message::Binary(init.clone().into())).await?;
        let ack = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await?
            .ok_or_else(|| anyhow!("Management handshake closed"))??;
        let Message::Binary(ack) = ack else {
            return Err(anyhow!("Encrypted management acknowledgement required"));
        };
        let mut stream = plain_rs::services::peers::management_wire::acknowledge(
            &key,
            &self.client,
            &init,
            &ack,
        )?;
        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(20),
            Duration::from_secs(20),
        );
        let mut pong_deadline = None;
        loop {
            self.current()?;
            tokio::select! {
                result = changed.changed() => { result?; self.current()?; },
                _ = heartbeat.tick() => {
                    socket.send(Message::Binary(plain_rs::services::peers::management_wire::encode(&ClientMessage::Ping { payload: None }, &key, &mut stream)?.into())).await?;
                    pong_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(10));
                },
                _ = async { if let Some(deadline) = pong_deadline { tokio::time::sleep_until(deadline).await } else { std::future::pending::<()>().await } } => return Err(anyhow!("Management heartbeat timeout")),
                command = commands.recv() => {
                    let command = command.ok_or_else(|| anyhow!("Subscription session closed"))?;
                    self.current()?;
                    socket.send(Message::Binary(plain_rs::services::peers::management_wire::encode(&command, &key, &mut stream)?.into())).await?;
                },
                frame = socket.next() => {
                    let frame = frame.ok_or_else(|| anyhow!("Management connection closed"))??;
                    match frame {
                        Message::Binary(frame) => {
                            let plain = stream.decode(&frame, &key).ok_or_else(|| anyhow!("Invalid management event authentication"))?;
                            let message: ServerMessage = serde_json::from_slice(&plain)?;
                            self.current()?;
                            if matches!(message, ServerMessage::Pong { .. }) { pong_deadline = None; }
                            else { outgoing.try_send(Ok(serde_json::to_string(&message)?)).map_err(|_| anyhow!("Subscription response backpressure"))?; }
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

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/subscriptions.rs"]
mod tests;
