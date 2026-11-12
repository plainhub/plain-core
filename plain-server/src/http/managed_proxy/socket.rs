use super::{PeerRoute, authorize_graphql, caller, current, decode_key};
use crate::server::ServerState;
use futures_util::{SinkExt, StreamExt};
use plain_libs::axum::{
    extract::{FromRequestParts, Query, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use plain_libs::protocols::websocket::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use plain_rs::{
    features::services::peers::subscriptions::PeerSubscription,
    infrastructure::{
        db::SessionRow,
        host::request_scope::NativeRequestScope,
        protocols::graphql::{
            PeerSubscriptionTarget,
            messages::{ClientMessage, ServerMessage},
        },
        protocols::websocket::Stream,
    },
};
use std::{collections::HashSet, time::Duration};

#[derive(serde::Deserialize)]
struct Parameters {
    cid: String,
}

pub(super) async fn upgrade(state: ServerState, target: String, request: Request) -> Response {
    let Ok(Query(parameters)) = Query::<Parameters>::try_from_uri(request.uri()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(session) = caller(&state, &parameters.cid) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(key) = decode_key(&session.token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let (mut parts, _) = request.into_parts();
    let Ok(socket) = WebSocketUpgrade::from_request_parts(&mut parts, &state).await else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    socket
        .max_message_size(1024 * 1024)
        .max_frame_size(1024 * 1024)
        .on_upgrade(move |socket| run(socket, state, target, session, key))
        .into_response()
}

async fn run(
    mut socket: WebSocket,
    state: ServerState,
    target: String,
    session: SessionRow,
    key: Vec<u8>,
) {
    let result = async {
        let (mut stream, acknowledgement) =
            crate::http::websocket::graphql::handshake(&mut socket, &session.client_id, &key)
                .await?;
        send(&mut socket, Message::Binary(acknowledgement.into())).await?;
        let first = tokio::time::timeout(Duration::from_secs(10), socket.next())
            .await
            .map_err(|_| "Subscription handshake timeout")?
            .ok_or("Subscription closed")?
            .map_err(|error| error.to_string())?;
        let Message::Binary(first) = first else {
            return Err("Encrypted subscription frame required".into());
        };
        let first = stream
            .decode(&first, &key)
            .ok_or("Invalid encrypted frame")?;
        if !first.is_empty() {
            return Err("Empty encrypted confirmation required".into());
        }
        let route = state
            .managed_proxy
            .route(&state, &target)
            .await
            .map_err(|_| "Device unavailable")?;
        current(&state, &session).map_err(|_| "Session revoked")?;
        let scope = NativeRequestScope::default();
        let upstream = PeerSubscription::open_with_pin(
            state.core.clone(),
            PeerSubscriptionTarget {
                peer_id: route.peer.id.clone(),
                public_key: route.peer.public_key.clone(),
            },
            scope.clone(),
            Some(route.pin),
        )?;
        let result = async {
            relay(
                &mut socket,
                &state,
                &session,
                &route,
                &upstream,
                stream,
                &key,
            )
            .await
        }
        .await;
        scope.close();
        upstream.close();
        result
    };
    let mut stop = state.stop.clone();
    let result = tokio::select! {
        biased;
        _ = stop.wait_for(|stopped| *stopped) => Err("Server stopped".to_owned()),
        result = result => result,
    };
    let _ = send(
        &mut socket,
        Message::Close(Some(CloseFrame {
            code: (if result.is_ok() { 1000u16 } else { 1008u16 }).into(),
            reason: (if result.is_ok() {
                ""
            } else {
                "managed_subscription_closed"
            })
            .into(),
        })),
    )
    .await;
}

async fn relay(
    socket: &mut WebSocket,
    state: &ServerState,
    session: &SessionRow,
    route: &PeerRoute,
    upstream: &PeerSubscription,
    mut stream: Stream,
    key: &[u8],
) -> Result<(), String> {
    let mut active = HashSet::<String>::new();
    let mut recheck = tokio::time::interval(Duration::from_secs(1));
    recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        current(state, session).map_err(|_| "Session revoked")?;
        route
            .check_current(state)
            .map_err(|_| "Management session revoked")?;
        tokio::select! {
            _ = recheck.tick() => {}
            incoming = socket.next() => {
                let plain = match incoming {
                    Some(Ok(Message::Binary(frame))) => stream.decode(&frame, key).ok_or("Invalid encrypted frame")?,
                    Some(Ok(Message::Ping(bytes))) => { send(socket, Message::Pong(bytes)).await?; continue; },
                    Some(Ok(Message::Pong(_))) => continue,
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    _ => return Err("Invalid subscription frame".into()),
                };
                let message = plain_libs::serde_json::from_slice::<ClientMessage>(&plain).map_err(|_| "Invalid subscription request")?;
                match &message {
                    ClientMessage::Subscribe { id, payload } => {
                        if active.len() >= 64 || active.contains(id) {
                            return Err("Subscription limit reached".into());
                        }
                        authorize_graphql(payload, true).map_err(|_| "Subscription denied")?;
                        active.insert(id.clone());
                    }
                    ClientMessage::Complete { id } => { active.remove(id); }
                    ClientMessage::Ping { payload } => {
                        let pong = plain_libs::serde_json::to_vec(&ServerMessage::Pong { payload: payload.clone() })
                            .map_err(|error| error.to_string())?;
                        let frame = stream.encode(&pong, key).ok_or("Sequence exhausted")?;
                        send(socket, Message::Binary(frame.into())).await?;
                        continue;
                    }
                    ClientMessage::Pong { .. } => continue,
                }
                upstream.send(std::str::from_utf8(&plain).map_err(|_| "Invalid subscription request")?)?;
            }
            message = upstream.read() => {
                let message = message?;
                let parsed = plain_libs::serde_json::from_str::<ServerMessage>(&message).map_err(|_| "Invalid subscription response")?;
                let id = match &parsed {
                    ServerMessage::Next { id, .. } => Some(id),
                    ServerMessage::Error { id, .. } | ServerMessage::Complete { id } => {
                        if !active.remove(id) { continue; }
                        None
                    }
                    ServerMessage::Pong { .. } => None,
                };
                current(state, session).map_err(|_| "Session revoked")?;
                route.check_current(state).map_err(|_| "Management session revoked")?;
                if id.is_some_and(|id| !active.contains(id)) {
                    continue;
                }
                let frame = stream.encode(message.as_bytes(), key).ok_or("Sequence exhausted")?;
                send(socket, Message::Binary(frame.into())).await?;
            }
        }
    }
}

async fn send(socket: &mut WebSocket, message: Message) -> Result<(), String> {
    tokio::time::timeout(Duration::from_secs(2), socket.send(message))
        .await
        .map_err(|_| "Slow subscription client")?
        .map_err(|error| error.to_string())
}
