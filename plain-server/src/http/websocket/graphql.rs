use crate::graphql::executor::RequestContext;
use crate::server::ServerState;
use futures_util::{SinkExt, StreamExt};
use plain_libs::axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use plain_libs::protocols::websocket::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use plain_rs::infrastructure::host::request_scope::NativeRequestScope;
use plain_rs::infrastructure::protocols::websocket as graphql;
use plain_rs::infrastructure::protocols::websocket::Role;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub(crate) struct Params {
    cid: String,
}
pub(crate) async fn upgrade(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
    ws: WebSocketUpgrade,
) -> Response {
    if params.cid.is_empty() || params.cid.len() > 1024 {
        return StatusCode::BAD_REQUEST.into_response();
    }
    ws.max_message_size(1024 * 1024)
        .max_frame_size(1024 * 1024)
        .on_upgrade(move |socket| run(socket, state, params.cid))
        .into_response()
}
struct Connection {
    session: std::sync::Arc<crate::graphql::subscriptions::session::Session>,
    stream: graphql::Stream,
    key: Vec<u8>,
}

pub(crate) async fn handshake(
    socket: &mut WebSocket,
    client: &str,
    key: &[u8],
) -> Result<(graphql::Stream, Vec<u8>), String> {
    let (init, ack) = acknowledge(socket, client, key, "/graphql").await?;
    let stream = graphql::Stream::new("/graphql", client, &init, &ack, Role::Server)
        .ok_or("Invalid binding")?;
    Ok((stream, ack))
}

pub(crate) async fn acknowledge(
    socket: &mut WebSocket,
    client: &str,
    key: &[u8],
    path: &str,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let first = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .map_err(|_| "Handshake timeout")?
        .ok_or("Socket closed")?
        .map_err(|error| error.to_string())?;
    let Message::Binary(init_frame) = first else {
        return Err("Encrypted handshake required".into());
    };
    let ack =
        graphql::accept_registration(key, client, path, &init_frame).ok_or("Invalid handshake")?;
    Ok((init_frame.to_vec(), ack))
}

async fn establish(
    socket: &mut WebSocket,
    state: &ServerState,
    client: String,
) -> Result<Connection, String> {
    if !state.prefs.get_or("remote_access", true) {
        return Err("Access disabled".into());
    }
    let local = plain_rs::features::services::sessions::local_key(state, &client);
    let key = local
        .clone()
        .or_else(|| {
            plain_rs::features::services::sessions::key(&state.db, &client)
                .ok()
                .flatten()
        })
        .ok_or("Session unavailable")?;
    let (mut stream, ack) = handshake(socket, &client, &key).await?;
    tokio::time::timeout(
        Duration::from_secs(2),
        socket.send(Message::Binary(ack.into())),
    )
    .await
    .map_err(|_| "Slow subscription client")?
    .map_err(|error| error.to_string())?;
    let first = tokio::time::timeout(Duration::from_secs(10), socket.next())
        .await
        .map_err(|_| "Handshake timeout")?
        .ok_or("Socket closed")?
        .map_err(|error| error.to_string())?;
    let Message::Binary(first) = first else {
        return Err("Encrypted confirmation required".into());
    };
    let first = stream
        .decode(&first, &key)
        .ok_or("Invalid encrypted confirmation")?;
    if !first.is_empty() {
        return Err("Empty encrypted confirmation required".into());
    }
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
        }) => {
            let result = async {
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
                    tokio::time::timeout(Duration::from_secs(2), socket.send(Message::Binary(frame.into()))).await.map_err(|_| "Slow subscription client")?.map_err(|error| error.to_string())?;
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
            code: (if result.is_err() { 1008u16 } else { 1000u16 }).into(),
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

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/websocket/graphql.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/websocket/no_subprotocol.rs"]
mod no_subprotocol_tests;
