use crate::server::ServerState;
use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use plain_rs::services::peers::status_runtime::{ConnectionGuard, emit};
use serde::Deserialize;
use serde_json::json;
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
        authenticated=tokio::time::timeout(Duration::from_secs(10), async {
            let challenge = uuid::Uuid::new_v4().to_string();
            socket.send(Message::Text(json!({"challenge": challenge}).to_string().into())).await.ok()?;
            match socket.recv().await {
                Some(Ok(Message::Binary(body))) => state.peer_status.connections.open(&state.db, &id, &challenge, &body).ok(),
                _ => None,
            }
        }) => authenticated.ok().flatten(),
    };
    let Some((lease, changed)) = authenticated else {
        let _ = socket.close().await;
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
    let sent = tokio::select! {_=stop.changed()=>false,_=public_stop.changed()=>false,result=socket.send(Message::Text("ok".into()))=>result.is_ok()};
    if sent {
        loop {
            tokio::select! {
                _=stop.changed()=>break,
                _=public_stop.changed()=>break,
                _=check.tick()=>if !state.peer_status.connections.valid(&state.db,&lease) { break; },
                frame=socket.recv()=>match frame {
                    Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,
                    _=>{},
                }
            }
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close()).await;
}
#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/unit/http/websocket/status.rs"]
mod tests;
