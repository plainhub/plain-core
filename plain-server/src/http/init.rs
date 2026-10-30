use crate::assembly::server::ServerState;
use plain_rs::prefs::security::PasswordType;
use axum::{Json, body::Bytes, extract::{ConnectInfo, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Response}};
use serde_json::json;
use std::net::SocketAddr;

pub(crate) async fn init(
    State(state): State<ServerState>,
    ConnectInfo(_remote): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let client_id = headers
        .get("c-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if client_id.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    if !state.prefs.get_or("desktop_access", true) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let key = crate::services::sessions::local_key(&state, client_id).or(
        crate::services::sessions::key(&state.db, client_id)
            .ok()
            .flatten(),
    );
    let authenticated = key
        .as_ref()
        .is_some_and(|key| plain_rs::crypto::xchacha_decrypt_raw(key, &body).is_some());
    let password_required = match plain_rs::prefs::security::password_type(&state.prefs) {
        Ok(value) => value == PasswordType::Fixed,
        Err(error) => {
            log::error!("Invalid password type: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let pair = match crate::services::peers::peer_wire::signing_keypair(&state.prefs) {
        Ok(pair) => pair,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    Json(json!({"signaturePublicKey":plain_rs::utils::base64::base64_encode(&pair[32..]),"authenticated":authenticated,"passwordRequired":password_required}))
        .into_response()
}

#[cfg(test)]
#[path = "../../tests/unit/http/init.rs"]
mod tests;
