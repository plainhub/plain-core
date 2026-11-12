use crate::server::ServerState;
use futures_util::{SinkExt, StreamExt};
use plain_libs::axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use plain_libs::protocols::websocket::{Message, WebSocket, WebSocketUpgrade};
use plain_rs::features::services::peers::status_runtime::{ConnectionGuard, emit};
use plain_rs::infrastructure::protocols::websocket::{self as crypto, Role};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct PublicState {
    pub(crate) state: ServerState,
    pub(crate) stop: tokio::sync::watch::Receiver<bool>,
}
#[derive(Deserialize)]
pub(crate) struct Parameters {
    cid: Option<String>,
}
pub(crate) async fn public(
    State(public): State<PublicState>,
    Query(query): Query<Parameters>,
    ws: WebSocketUpgrade,
) -> Response {
    let state = public.state.clone();
    if *state.stop.borrow() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(id) = query.cid.filter(|id| !id.is_empty() && id.len() <= 1024) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(permit) = state.peer_status_capacity.clone().try_acquire_owned() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    ws.max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            connected(state, public.stop, id, socket).await;
        })
}
async fn connected(
    state: ServerState,
    mut public_stop: tokio::sync::watch::Receiver<bool>,
    id: String,
    mut socket: WebSocket,
) {
    let mut stop = state.stop.clone();
    let authenticated = tokio::select! {
        _=stop.changed()=>None,
        _=public_stop.changed()=>None,
        result=tokio::time::timeout(Duration::from_secs(10), async {
            let peer = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &id).ok()??;
            if !peer.is_paired() { return None; }
            let key = plain_libs::utils::base64::base64_decode(&peer.key);
            let (init, ack) = super::graphql::acknowledge(&mut socket, &id, &key, "/status").await.ok()?;
            let mut stream = crypto::Stream::new("/status", &id, &init, &ack, Role::Server)?;
            socket.send(Message::Binary(ack.clone().into())).await.ok()?;
            let Some(Ok(Message::Binary(frame))) = socket.next().await else { return None; };
            let signature = stream.decode(&frame, &key)?;
            if !plain_rs::features::peers::status::verify(&peer, &init, &ack, &signature) { return None; }
            let admitted = state.peer_status.connections.admit(&state.db, peer).ok()?;
            Some((admitted, key, stream))
        }) => result.ok().flatten(),
    };
    let Some(((lease, changed), key, mut stream)) = authenticated else {
        let _ = socket.close(None).await;
        return;
    };
    let _guard = ConnectionGuard {
        state: state.core.clone(),
        lease: lease.clone(),
    };
    if changed {
        emit(&state, &id, true);
    }
    let mut check = tokio::time::interval(Duration::from_secs(1));
    let Some(confirmation) = stream.encode(b"ok", &key) else {
        return;
    };
    let sent = tokio::select! {_=stop.changed()=>false,_=public_stop.changed()=>false,result=socket.send(Message::Binary(confirmation.into()))=>result.is_ok()};
    if sent {
        loop {
            tokio::select! {
                _=stop.changed()=>break,
                _=public_stop.changed()=>break,
                _=check.tick()=>if !state.peer_status.connections.valid(&state.db,&lease) { break; },
                frame=socket.next()=>match frame {
                    Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,
                    Some(Ok(Message::Binary(frame))) => if stream.decode(&frame, &key).is_none() { break; },
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {},
                    _=>break,
                }
            }
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close(None)).await;
}
#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../tests/plain-server/unit/http/websocket/status.rs"]
mod tests;
