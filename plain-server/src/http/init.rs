use crate::server::ServerState;
use axum::{
    Json,
    body::Bytes,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use plain_rs::features::prefs::security::PasswordType;
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
    if !state.prefs.get_or("remote_access", true) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let key = plain_rs::features::services::sessions::local_key(&state, client_id).or(
        plain_rs::features::services::sessions::key(&state.db, client_id)
            .ok()
            .flatten(),
    );
    let authenticated = key
        .as_ref()
        .and_then(|key| {
            plain_rs::infrastructure::transport::http_request::decrypt_raw(
                key, "POST", "/init", &body,
            )
        })
        .and_then(|plain| String::from_utf8(plain).ok())
        .is_some_and(|plain| {
            state
                .main_replay
                .admit(
                    &state.db,
                    client_id,
                    &plain,
                    chrono::Utc::now().timestamp_millis(),
                )
                .is_ok()
        });
    let password_required = match plain_rs::features::prefs::security::password_type(&state.prefs) {
        Ok(value) => value == PasswordType::Fixed,
        Err(error) => {
            log::error!("Invalid password type: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let pair = match plain_rs::features::services::peers::peer_wire::signing_keypair(&state.prefs) {
        Ok(pair) => pair,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let mut result = json!({"signaturePublicKey":plain_libs::utils::base64::base64_encode(&pair[32..]),"authenticated":authenticated,"passwordRequired":password_required});
    if password_required {
        match plain_rs::features::prefs::security::opaque_public_key(&state.prefs) {
            Ok(Some(key)) => result["opaquePublicKey"] = json!(key),
            Ok(None) => {}
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
    #[cfg(feature = "http_transport")]
    if let Some(server) = state.public_server.lock().await.as_ref() {
        let pin = &server.tls_certificate_sha256;
        let proof = format!("plain-peer-tls-v1:{pin}");
        result["tlsCertificateSha256"] = json!(pin);
        result["tlsCertificateSignature"] =
            json!(plain_libs::crypto::ed25519_sign(&pair, proof.as_bytes()));
    }
    Json(result).into_response()
}

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/http/init.rs"]
mod tests;
