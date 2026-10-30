use super::pairing_attempts::Resolved;
use crate::assembly::server::ServerState;
use anyhow::{Result, ensure};
use futures_util::{SinkExt, StreamExt};
use plain_rs::{
    crypto::{self, EcdhSession},
    utils::base64::{base64_decode, base64_encode},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::sync::mpsc;

pub(super) async fn run(
    state: &ServerState,
    generation: &str,
    target: Resolved,
    password: Option<String>,
    mut passwords: mpsc::Receiver<String>,
) -> Result<()> {
    let base = target
        .url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No management address available"))?;
    let client_id = state.native_client_id().map_err(anyhow::Error::msg)?;
    let saved = if target.target.device_id.is_empty() {
        None
    } else {
        plain_rs::db::chat_store::peers::get(&state.db, &target.target.device_id)?
    };
    let response = state
        .lan
        .client
        .post(base.join("init")?)
        .header("c-id", &client_id)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| anyhow::anyhow!("failed_connect"))?;
    ensure!(response.status().is_success(), "desktop_access_disabled");
    let init: Value =
        serde_json::from_slice(&crate::services::peers::peer_lan::limited(response, 8192).await?)?;
    let public_key = init["signaturePublicKey"]
        .as_str()
        .filter(|k| base64_decode(k).len() == 32)
        .ok_or_else(|| anyhow::anyhow!("signature_verification_failed"))?
        .to_owned();
    if let Some(saved) = &saved {
        check_identity(&saved.id, &saved.public_key, &saved.id, &public_key)?;
    }
    if let Some(saved) = saved
        .as_ref()
        .filter(|p| !p.token.is_empty() && p.is_paired())
    {
        if let Ok(metadata) = metadata(state, base, &client_id, &base64_decode(&saved.token)).await
        {
            check_identity(
                &saved.id,
                &saved.public_key,
                text(&metadata, "clientId")?,
                &public_key,
            )?;
            let _commit = state.pairing_runtime.commit.lock().unwrap();
            ensure!(
                state
                    .pairing_runtime
                    .attempts
                    .get(generation)
                    .is_some_and(|s| !matches!(s.phase.as_str(), "CANCELLED" | "FAILED")),
                "cancelled"
            );
            let name = text(&metadata, "deviceName")?;
            let changed = state.db.with_conn(|connection| {
                plain_rs::db::chat_store::peers::update_name(connection, &saved.id, name)
            })?;
            if changed > 0 {
                let _ = state.events.send(plain_rs::events::WsEvent::broadcast(
                    plain_rs::chat::events::WS_PEER_UPDATED,
                    json!({"id":saved.id}).to_string(),
                ));
            }
            state.pairing_runtime.attempts.transition(
                state,
                generation,
                "SUCCEEDED",
                None,
                Some(&saved.id),
            );
            return Ok(());
        }
    }
    let password_required = init["passwordRequired"]
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))?;
    let digest = if password_required {
        let password = match password {
            Some(password) => password,
            None => {
                state.pairing_runtime.attempts.transition(
                    state,
                    generation,
                    "PASSWORD_REQUIRED",
                    None,
                    None,
                );
                passwords
                    .recv()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("cancelled"))?
            }
        };
        Some(plain_rs::crypto::sha512_hex(&password))
    } else {
        None
    };
    let local = super::pairing_runtime::local_device(state).await?;
    ensure!(local.port != 0, "chat_pairing_unavailable");
    let signing = crate::services::peers::peer_wire::signing_keypair(&state.prefs)?;
    let ecdh = EcdhSession::generate();
    let startup = state.host.startup();
    let mut request = json!({
        "browserName":"PlainApp","browserVersion":startup.app_version,
        "osName":startup.os_name,"osVersion":startup.os_version,
        "isMobile":matches!(startup.device_type, plain_rs::chat::enums::DeviceType::Phone | plain_rs::chat::enums::DeviceType::Tablet),
        "ecdhPublicKey":base64_encode(&ecdh.public_key_bytes),
        "peer":{"deviceName":local.name,"port":local.port,"deviceType":local.device_type,"ips":local.ips,"signaturePublicKey":base64_encode(&signing[32..])}
    });
    if let Some(digest) = &digest {
        request["password"] = json!(digest);
    }
    let mut url = base.join("auth")?;
    url.set_scheme(if base.scheme() == "https" {
        "wss"
    } else {
        "ws"
    })
    .map_err(|_| anyhow::anyhow!("Invalid management address"))?;
    url.query_pairs_mut()
        .append_pair("cid", &client_id)
        .append_pair("auth", if password_required { "1" } else { "2" });
    state
        .pairing_runtime
        .attempts
        .transition(state, generation, "AUTHENTICATING", None, None);
    let mut socket = crate::services::peers::status_socket::connect(url.as_str())
        .await
        .map_err(|_| anyhow::anyhow!("failed_connect_ws"))?;
    let mode = if password_required {
        crate::http::auth::login_channel::LoginMode::Password
    } else {
        crate::http::auth::login_channel::LoginMode::DeviceConfirmation
    };
    let (client, hello) = crate::http::auth::login_channel::Client::start(&client_id, mode)?;
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&hello)?.into(),
        ))
        .await?;
    let response = loop {
        use tokio_tungstenite::tungstenite::Message;
        match socket
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("failed_connect_ws"))??
        {
            Message::Text(text) => {
                break serde_json::from_str::<crate::http::auth::login_channel::ServerHello>(
                    &text,
                )?;
            }
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
            _ => anyhow::bail!("invalid_auth_response"),
        }
    };
    let (channel, reveal) = client.accept(
        &response,
        &public_key,
        &target.target.device_id,
        chrono::Utc::now().timestamp_millis() as u64,
    )?;
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&reveal)?.into(),
        ))
        .await?;
    request["loginContext"] = json!(channel.context);
    if !password_required {
        state.pairing_runtime.attempts.set_verification_code(
            state,
            generation,
            &channel.verification_code,
        );
    }

    let request_bytes = serde_json::to_vec(&request)?;
    let encrypted = if let Some(digest) = &digest {
        let password_encrypted =
            crypto::xchacha_encrypt_raw(&digest.as_bytes()[..32], &request_bytes)
                .ok_or_else(|| anyhow::anyhow!("invalid_password"))?;
        channel.encrypt_request(
            &password_encrypted,
            chrono::Utc::now().timestamp_millis() as u64,
        )?
    } else {
        channel.encrypt_request(&request_bytes, chrono::Utc::now().timestamp_millis() as u64)?
    };
    socket
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            encrypted.into(),
        ))
        .await
        .map_err(|_| anyhow::anyhow!("failed_connect_ws"))?;
    let response = loop {
        let frame = socket
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("failed_connect_ws"))?
            .map_err(|_| anyhow::anyhow!("failed_connect_ws"))?;
        use tokio_tungstenite::tungstenite::Message;
        match frame {
            Message::Binary(data) => {
                let decrypted = channel
                    .decrypt_response(&data, chrono::Utc::now().timestamp_millis() as u64)?;
                let plain = if let Some(digest) = &digest {
                    crypto::xchacha_decrypt_raw(&digest.as_bytes()[..32], &decrypted)
                        .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))?
                } else {
                    decrypted
                };
                let response: Value = serde_json::from_slice(&plain)
                    .map_err(|_| anyhow::anyhow!("invalid_auth_response"))?;
                if response["status"] == "PENDING" {
                    state.pairing_runtime.attempts.transition(
                        state,
                        generation,
                        "WAITING_CONFIRMATION",
                        None,
                        None,
                    );
                    continue;
                }
                break response;
            }
            Message::Ping(data) => socket
                .send(Message::Pong(data))
                .await
                .map_err(|_| anyhow::anyhow!("failed_connect_ws"))?,
            Message::Close(reason) => {
                let reason = reason.map(|r| r.reason.to_string()).unwrap_or_default();
                anyhow::bail!(match reason.as_str() {
                    "invalid_password" => "invalid_password",
                    "too_many_login_attempts" => "too_many_login_attempts",
                    "rejected" => "rejected",
                    _ => "failed_connect_ws",
                });
            }
            _ => anyhow::bail!("invalid_auth_response"),
        }
    };
    ensure!(
        response["loginContext"].as_str() == Some(channel.context.as_str()),
        "invalid_auth_response"
    );
    let remote_id = verify_response(
        &response,
        &target.target.device_id,
        &public_key,
        chrono::Utc::now().timestamp_millis(),
    )?;
    if let Some(saved) = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)? {
        check_identity(&saved.id, &saved.public_key, &remote_id, &public_key)?;
    }
    {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        state
            .pairing_runtime
            .attempts
            .claim_identity(Some(state), generation, &remote_id)?;
    }
    let shared = ecdh
        .compute_shared_key(&base64_decode(text(&response, "ecdhPublicKey")?))
        .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))?;
    let metadata = metadata(state, base, &client_id, &shared).await?;
    ensure!(
        text(&metadata, "clientId")? == remote_id,
        "peer_identity_changed"
    );
    let mut material = b"plain-chat-pairing-v1".to_vec();
    material.extend(shared);
    let chat_key = base64_encode(&Sha256::digest(material));
    let token = base64_encode(&shared);
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    ensure!(
        state
            .pairing_runtime
            .attempts
            .get(generation)
            .is_some_and(|s| !matches!(s.phase.as_str(), "CANCELLED" | "FAILED")),
        "cancelled"
    );
    if let Some(saved) = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)? {
        check_identity(&saved.id, &saved.public_key, &remote_id, &public_key)?;
    }
    state.pairing_runtime.attempts.transition(
        state,
        generation,
        "COMMITTING",
        None,
        Some(&remote_id),
    );
    let device_type = text(&metadata, "deviceType")?
        .parse()
        .unwrap_or(target.device_type);
    let port = if base.scheme() == "http" {
        metadata["httpsPort"]
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port != 0)
            .ok_or_else(|| anyhow::anyhow!("No secure peer address available"))?
    } else {
        target.target.device_port
    };
    state
        .db
        .login_peer(
            &remote_id,
            text(&metadata, "deviceName")?,
            &target.ips.join(","),
            port,
            device_type,
            &token,
            &public_key,
            &chat_key,
        )
        .map_err(|_| anyhow::anyhow!("Failed to save paired device"))?;
    state.pairing_runtime.attempts.transition(
        state,
        generation,
        "SUCCEEDED",
        None,
        Some(&remote_id),
    );
    if let Some(peer) = plain_rs::db::chat_store::peers::get(&state.db, &remote_id)? {
        super::pairing_runtime::success(
            state,
            &serde_json::to_value(crate::services::peers::peer_address::view(&peer))?,
            &peer.ip,
        );
    }
    Ok(())
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))
}
fn check_identity(expected_id: &str, expected_key: &str, id: &str, key: &str) -> Result<()> {
    ensure!(
        (expected_id.is_empty() || expected_id == id)
            && (expected_key.is_empty() || expected_key == key),
        "peer_identity_changed"
    );
    Ok(())
}
fn verify_response(response: &Value, expected: &str, public_key: &str, now: i64) -> Result<String> {
    let id = text(response, "clientId")?;
    ensure!(
        !id.is_empty() && id.len() <= 256 && (expected.is_empty() || expected == id),
        "peer_identity_changed"
    );
    ensure!(
        response["status"] == "COMPLETED" && response["chatPaired"] == true,
        "chat_pairing_unavailable"
    );
    let timestamp = response["timestamp"]
        .as_i64()
        .ok_or_else(|| anyhow::anyhow!("signature_verification_failed"))?;
    ensure!(
        (i128::from(now) - i128::from(timestamp)).abs() <= 300_000,
        "signature_verification_failed"
    );
    let mut data = format!(
        "{id}|COMPLETED|{}|{timestamp}|true",
        text(response, "ecdhPublicKey")?
    );
    if let Some(context) = response.get("loginContext") {
        data.push('|');
        data.push_str(
            context
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("invalid_auth_response"))?,
        );
    }
    ensure!(
        crypto::ed25519_verify(public_key, data.as_bytes(), text(response, "signature")?),
        "signature_verification_failed"
    );
    Ok(id.into())
}
async fn metadata(
    state: &ServerState,
    base: &reqwest::Url,
    client_id: &str,
    key: &[u8],
) -> Result<Value> {
    let data = crate::services::peers::management_http::query(
        state,
        base,
        client_id,
        key,
        json!({"query":"{ app { clientId deviceName deviceType httpsPort } }"}),
        65536,
    )
    .await
    .map_err(|error| {
        if error.downcast_ref::<reqwest::Error>().is_some() {
            anyhow::anyhow!("failed_connect")
        } else {
            anyhow::anyhow!("invalid_session")
        }
    })?;
    ensure!(data["app"].is_object(), "invalid_session");
    Ok(data["app"].clone())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/discovery/pairing_login.rs"]
mod tests;
