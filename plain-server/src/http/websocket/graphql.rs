use crate::graphql::executor::RequestContext;
use crate::server::ServerState;
use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use plain_rs::host::request_scope::NativeRequestScope;
use plain_rs::protocols::graphql::{self, Role};
use plain_rs::protocols::graphql::{Ack, Init, PROTOCOL};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub(crate) struct Params {
    cid: String,
}
pub(crate) async fn upgrade(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let protocol = headers
        .get("sec-websocket-protocol")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|item| item.trim() == PROTOCOL));
    if !protocol || params.cid.is_empty() || params.cid.len() > 1024 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    ws.protocols([PROTOCOL])
        .max_message_size(1024 * 1024)
        .max_frame_size(1024 * 1024)
        .on_upgrade(move |socket| run(socket, state, params.cid))
        .into_response()
}
struct Connection {
    session: std::sync::Arc<crate::graphql::subscriptions::session::Session>,
    stream: graphql::Stream,
    key: Vec<u8>,
    ack: Vec<u8>,
}
async fn establish(
    socket: &mut WebSocket,
    state: &ServerState,
    client: String,
) -> Result<Connection, String> {
    if !state.prefs.get_or("desktop_access", true) {
        return Err("Access disabled".into());
    }
    let local = plain_rs::services::sessions::local_key(state, &client);
    let key = local
        .clone()
        .or_else(|| {
            plain_rs::services::sessions::key(&state.db, &client)
                .ok()
                .flatten()
        })
        .ok_or("Session unavailable")?;
    let first = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .map_err(|_| "Handshake timeout")?
        .ok_or("Socket closed")?
        .map_err(|error| error.to_string())?;
    let Message::Binary(init_frame) = first else {
        return Err("Encrypted handshake required".into());
    };
    let plain = graphql::open_init(&key, &client, &init_frame).ok_or("Invalid handshake")?;
    let init: Init = serde_json::from_slice(&plain).map_err(|error| error.to_string())?;
    if init.client_nonce.len() != 64
        || !init
            .client_nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Invalid nonce".into());
    }
    state
        .main_replay
        .admit(
            &client,
            &format!("{}|{}|", init.timestamp_millis, init.client_nonce),
            chrono::Utc::now().timestamp_millis(),
        )
        .map_err(|_| "Invalid handshake")?;
    let ack = Ack {
        server_nonce: plain_rs::utils::hex::bytes_to_hex(&plain_rs::crypto::random_bytes(32)),
    };
    let ack_frame = graphql::seal_ack(
        &key,
        &client,
        &init_frame,
        &serde_json::to_vec(&ack).map_err(|error| error.to_string())?,
    )
    .ok_or("Invalid acknowledgement")?;
    let stream = graphql::Stream::new(&client, &init_frame, &ack_frame, Role::Server)
        .ok_or("Invalid binding")?;
    let context = if local.is_some() {
        RequestContext::authenticated_local(state.clone(), client.clone())
    } else {
        RequestContext::authenticated_remote(state.clone(), client.clone())
    };
    let session = state.subscription_sessions.open(
        state.clone(),
        context,
        client,
        Some(key.clone()),
        NativeRequestScope::default(),
    )?;
    Ok(Connection {
        session,
        stream,
        key,
        ack: ack_frame,
    })
}
async fn run(mut socket: WebSocket, state: ServerState, client: String) {
    let mut stop = state.stop.clone();
    let established = tokio::select! {
        _ = async { let _ = stop.wait_for(|value| *value).await; } => return,
        established = establish(&mut socket, &state, client) => established,
    };
    let result = match established {
        Ok(Connection {
            session,
            mut stream,
            key,
            ack,
        }) => {
            let result = async {
                tokio::time::timeout(Duration::from_secs(2), socket.send(Message::Binary(ack))).await.map_err(|_| "Slow subscription client")?.map_err(|error| error.to_string())?;
                session.mark_online()?;
                let mut read = Box::pin(session.read());
                loop {
                    let outbound = tokio::select! {
                        _ = async { let _ = stop.wait_for(|value| *value).await; } => break,
                        incoming = socket.next() => match incoming {
                            Some(Ok(Message::Binary(frame))) => {
                                let plain = stream.decode(&frame, &key).ok_or("Invalid encrypted frame")?;
                                session.send(std::str::from_utf8(&plain).map_err(|error| error.to_string())?)?;
                                continue;
                            }
                            Some(Ok(Message::Ping(bytes))) => {
                                tokio::time::timeout(Duration::from_secs(2), socket.send(Message::Pong(bytes))).await.map_err(|_| "Slow subscription client")?.map_err(|error| error.to_string())?;
                                continue;
                            }
                            Some(Ok(Message::Pong(_))) => continue,
                            Some(Ok(Message::Close(_))) | None => break,
                            _ => return Err("Invalid subscription frame".to_owned()),
                        },
                        message = &mut read => message?,
                    };
                    read = Box::pin(session.read());
                    let frame = stream.encode(outbound.as_bytes(), &key).ok_or("Sequence exhausted")?;
                    tokio::time::timeout(Duration::from_secs(2), socket.send(Message::Binary(frame))).await.map_err(|_| "Slow subscription client")?.map_err(|error| error.to_string())?;
                }
                Ok::<_, String>(())
            }.await;
            state.subscription_sessions.close(session.id);
            if let Err(error) = state
                .mdns
                .release_discovery_client(&state, session.client_id())
                .await
            {
                log::warn!("Discovery client cleanup: {error}");
            }
            result
        }
        Err(error) => Err(error),
    };
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        socket.send(Message::Close(Some(CloseFrame {
            code: if result.is_err() { 1008 } else { 1000 },
            reason: if result.is_err() {
                "invalid_subscription"
            } else {
                ""
            }
            .into(),
        }))),
    )
    .await;
}
