use crate::server::ServerState;
use axum::{
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

pub(crate) async fn call(
    State(state): State<ServerState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let body = match std::str::from_utf8(&body) {
        Ok(body) => body,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid UTF-8").into_response(),
    };
    match plain_rs::features::services::discovery::pairing_runtime::handle_nearby_post(
        &state,
        body,
        remote.ip(),
        headers.get("c-id").and_then(|value| value.to_str().ok()),
    )
    .await
    {
        Ok(true) => (StatusCode::OK, "1").into_response(),
        Ok(false) => (StatusCode::BAD_REQUEST, "unknown message type").into_response(),
        Err(error) => {
            log::debug!("nearby request rejected: {error}");
            (StatusCode::BAD_REQUEST, "invalid nearby message").into_response()
        }
    }
}
