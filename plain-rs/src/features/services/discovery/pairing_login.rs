use crate::features::services::discovery::pairing_attempts::PairingPhase;
use crate::features::services::discovery::pairing_attempts::Resolved;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use futures_util::{SinkExt, StreamExt};
use plain_libs::protocols::http_client;
use plain_libs::utils::base64::base64_decode;
use plain_libs::utils::base64::base64_encode;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::sync::mpsc;

pub(super) async fn run(
    state: &CoreRuntime,
    generation: &str,
    target: Resolved,
    password: Option<String>,
    mut passwords: mpsc::Receiver<String>,
) -> Result<()> {
    let base = target
        .url
        .as_ref()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("No management address available"))?;
    let client_id = state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?;
    let saved = if target.target.device_id.is_empty() {
        None
    } else {
        crate::infrastructure::db::chat_store::peers::get(&state.db, &target.target.device_id)?
    };
    let response = state
        .lan
        .client
        .post(base.join("init")?)
        .header("c-id", &client_id)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| plain_libs::anyhow::anyhow!("failed_connect"))?;
    ensure!(response.status().is_success(), "remote_access_disabled");
    let init: Value = serde_json::from_slice(
        &crate::features::services::peers::peer_lan::limited(response, 8192).await?,
    )?;
    let public_key = init["signaturePublicKey"]
        .as_str()
        .filter(|k| base64_decode(k).len() == 32)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("signature_verification_failed"))?
        .to_owned();
    if let Some(saved) = saved
        .as_ref()
        .filter(|p| !p.token.is_empty() && p.is_paired() && p.public_key == public_key)
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
                    .is_some_and(|s| !matches!(
                        s.phase,
                        PairingPhase::Cancelled | PairingPhase::Failed
                    )),
                "cancelled"
            );
            let name = text(&metadata, "deviceName")?;
            let changed = state.db.with_conn(|connection| {
                crate::infrastructure::db::chat_store::peers::update_name(
                    connection, &saved.id, name,
                )
            })?;
            if changed > 0 {
                let _ = state
                    .events
                    .send(crate::infrastructure::events::WsEvent::broadcast(
                        crate::features::chat::events::WS_PEER_UPDATED,
                        json!({"id":saved.id}).to_string(),
                    ));
            }
            state.pairing_runtime.attempts.transition(
                state,
                generation,
                PairingPhase::Succeeded,
                None,
                Some(&saved.id),
            );
            return Ok(());
        }
    }
    let password_required = init["passwordRequired"]
        .as_bool()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response"))?;
    let changed = saved.as_ref().is_some_and(|p| p.public_key != public_key);
    if changed || (!password_required && saved.is_none()) {
        let digest = Sha256::digest(base64_decode(&public_key))
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut trust = state.pairing_runtime.attempts.trust_changes(generation)?;
        state
            .pairing_runtime
            .attempts
            .set_identity_changed(state, generation, changed);
        state
            .pairing_runtime
            .attempts
            .set_verification_code(state, generation, &digest);
        state.pairing_runtime.attempts.transition(
            state,
            generation,
            PairingPhase::TrustRequired,
            None,
            None,
        );
        while !*trust.borrow_and_update() {
            trust
                .changed()
                .await
                .map_err(|_| plain_libs::anyhow::anyhow!("cancelled"))?;
        }
    }
    let password = if password_required {
        let password = match password {
            Some(password) => password,
            None => {
                state.pairing_runtime.attempts.transition(
                    state,
                    generation,
                    PairingPhase::PasswordRequired,
                    None,
                    None,
                );
                passwords
                    .recv()
                    .await
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("cancelled"))?
            }
        };
        Some(password)
    } else {
        None
    };
    let local = crate::features::services::discovery::pairing_runtime::local_device(state).await?;
    ensure!(local.port != 0, "chat_pairing_unavailable");
    let signing = crate::features::services::peers::peer_wire::signing_keypair(&state.prefs)?;
    let startup = state.host.startup();
    let request = json!({
        "appName":"PlainApp","appVersion":startup.app_version,
        "osName":startup.os_name,"osVersion":startup.os_version,
        "isMobile":matches!(startup.device_type, crate::features::chat::enums::DeviceType::Phone | crate::features::chat::enums::DeviceType::Tablet),
        "peer":{"deviceName":local.name,"port":local.port,"deviceType":local.device_type,"ips":local.ips,"signaturePublicKey":base64_encode(&signing[32..])}
    });
    let mut url = base.join("auth")?;
    url.set_scheme(if base.scheme() == "https" {
        "wss"
    } else {
        "ws"
    })
    .map_err(|_| plain_libs::anyhow::anyhow!("Invalid management address"))?;
    url.query_pairs_mut().append_pair("cid", &client_id);
    state.pairing_runtime.attempts.transition(
        state,
        generation,
        PairingPhase::Authenticating,
        None,
        None,
    );
    let mut socket = crate::features::services::peers::status_socket::connect(url.as_str())
        .await
        .map_err(|_| plain_libs::anyhow::anyhow!("failed_connect_ws"))?;
    let protection = if let Some(password) = password {
        let client = plain_libs::auth::client::OpaqueLogin::start(password.into_bytes())
            .map_err(plain_libs::anyhow::Error::msg)?;
        socket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                client.request().into(),
            ))
            .await?;
        let reply = next_binary(&mut socket).await?;
        let result = client
            .finish(
                &reply,
                &client_id,
                &base64_decode(&public_key),
                &base64_decode(text(&init, "opaquePublicKey")?),
            )
            .map_err(plain_libs::anyhow::Error::msg)?;
        let key: [u8; 32] = result
            .api_key
            .as_slice()
            .try_into()
            .map_err(|_| plain_libs::anyhow::anyhow!("Invalid session key"))?;
        let encrypted = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
            &key,
            &serde_json::to_vec(&request)?,
            &result.request_aad,
        )
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Encryption failed"))?;
        let mut finish = result.ke3;
        finish.extend(encrypted);
        socket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                finish.into(),
            ))
            .await?;
        Protection::Password {
            key,
            aad: result.response_aad,
        }
    } else {
        let (client, hello) =
            crate::features::services::confirmation_channel::Client::start(&client_id)?;
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
                .ok_or_else(|| plain_libs::anyhow::anyhow!("failed_connect_ws"))??
            {
                Message::Text(text) => {
                    break serde_json::from_str::<
                        crate::features::services::confirmation_channel::ServerHello,
                    >(&text)?;
                }
                Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
                _ => plain_libs::anyhow::bail!("invalid_auth_response"),
            }
        };
        let channel = client.accept(
            &response,
            &public_key,
            &target.target.device_id,
            plain_libs::chrono::Utc::now().timestamp_millis() as u64,
        )?;
        state.pairing_runtime.attempts.set_verification_code(
            state,
            generation,
            &channel.verification_code,
        );
        let encrypted = channel.encrypt_request(
            &serde_json::to_vec(&request)?,
            plain_libs::chrono::Utc::now().timestamp_millis() as u64,
        )?;
        socket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                encrypted.into(),
            ))
            .await?;
        Protection::Confirmation(channel)
    };
    let response = loop {
        let frame = socket
            .next()
            .await
            .ok_or_else(|| plain_libs::anyhow::anyhow!("failed_connect_ws"))?
            .map_err(|_| plain_libs::anyhow::anyhow!("failed_connect_ws"))?;
        use tokio_tungstenite::tungstenite::Message;
        match frame {
            Message::Binary(data) => {
                let plain = protection.decrypt(&data)?;
                let response: Value = serde_json::from_slice(&plain)
                    .map_err(|_| plain_libs::anyhow::anyhow!("invalid_auth_response"))?;
                if response["status"] == "PENDING" {
                    state.pairing_runtime.attempts.transition(
                        state,
                        generation,
                        PairingPhase::WaitingConfirmation,
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
                .map_err(|_| plain_libs::anyhow::anyhow!("failed_connect_ws"))?,
            Message::Close(reason) => {
                let reason = reason.map(|r| r.reason.to_string()).unwrap_or_default();
                plain_libs::anyhow::bail!(match reason.as_str() {
                    "invalid_password" => "invalid_password",
                    "too_many_login_attempts" => "too_many_login_attempts",
                    "rejected" => "rejected",
                    _ => "failed_connect_ws",
                });
            }
            _ => plain_libs::anyhow::bail!("invalid_auth_response"),
        }
    };
    let remote_id = verify_response(&response, &target.target.device_id)?;
    {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        state
            .pairing_runtime
            .attempts
            .claim_identity(Some(state), generation, &remote_id)?;
    }
    let shared = protection.key();
    let metadata = metadata(state, base, &client_id, &shared).await?;
    ensure!(
        text(&metadata, "clientId")? == remote_id,
        "peer_identity_changed"
    );
    let mut material = b"chat-pairing".to_vec();
    material.extend(shared);
    let chat_key = base64_encode(&Sha256::digest(material));
    let token = base64_encode(&shared);
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    ensure!(
        state
            .pairing_runtime
            .attempts
            .get(generation)
            .is_some_and(|s| !matches!(s.phase, PairingPhase::Cancelled | PairingPhase::Failed)),
        "cancelled"
    );
    let current = crate::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)?;
    ensure!(
        current.as_ref().map(|p| &p.public_key) == saved.as_ref().map(|p| &p.public_key),
        "peer_identity_changed"
    );
    state.pairing_runtime.attempts.transition(
        state,
        generation,
        PairingPhase::Committing,
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
            .ok_or_else(|| plain_libs::anyhow::anyhow!("No secure peer address available"))?
    } else {
        target.target.device_port
    };
    state
        .db
        .login_peer_confirmed(
            &remote_id,
            text(&metadata, "deviceName")?,
            &target.ips.join(","),
            port,
            device_type,
            &token,
            &public_key,
            &chat_key,
            saved
                .as_ref()
                .map(|p| p.public_key.as_str())
                .unwrap_or_default(),
        )
        .map_err(|_| plain_libs::anyhow::anyhow!("Failed to save paired device"))?;
    state.pairing_runtime.attempts.transition(
        state,
        generation,
        PairingPhase::Succeeded,
        None,
        Some(&remote_id),
    );
    if let Some(peer) = crate::infrastructure::db::chat_store::peers::get(&state.db, &remote_id)? {
        crate::features::services::discovery::pairing_runtime::success(
            state,
            &serde_json::to_value(crate::features::services::peers::peer_address::view(&peer))?,
            &peer.ip,
            None,
        );
    }
    Ok(())
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response"))
}
fn check_identity(expected_id: &str, expected_key: &str, id: &str, key: &str) -> Result<()> {
    ensure!(
        (expected_id.is_empty() || expected_id == id)
            && (expected_key.is_empty() || expected_key == key),
        "peer_identity_changed"
    );
    Ok(())
}
fn verify_response(response: &Value, expected: &str) -> Result<String> {
    let id = text(response, "clientId")?;
    ensure!(
        !id.is_empty() && id.len() <= 256 && (expected.is_empty() || expected == id),
        "peer_identity_changed"
    );
    ensure!(
        response["status"] == "COMPLETED" && response["chatPaired"] == true,
        "chat_pairing_unavailable"
    );
    Ok(id.into())
}
enum Protection {
    Password { key: [u8; 32], aad: Vec<u8> },
    Confirmation(crate::features::services::confirmation_channel::Channel),
}
impl Protection {
    fn key(&self) -> [u8; 32] {
        match self {
            Self::Password { key, .. } => *key,
            Self::Confirmation(c) => c.api_key(),
        }
    }
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::Password { key, aad } => {
                plain_libs::crypto::xchacha_decrypt_raw_with_aad(key, data, aad)
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response"))
            }
            Self::Confirmation(c) => {
                c.decrypt_response(data, plain_libs::chrono::Utc::now().timestamp_millis() as u64)
            }
        }
    }
}
async fn next_binary<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<Vec<u8>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    use tokio_tungstenite::tungstenite::Message;
    loop {
        match socket
            .next()
            .await
            .ok_or_else(|| plain_libs::anyhow::anyhow!("failed_connect_ws"))??
        {
            Message::Binary(bytes) => return Ok(bytes.to_vec()),
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
            _ => plain_libs::anyhow::bail!("invalid_auth_response"),
        }
    }
}
async fn metadata(
    state: &CoreRuntime,
    base: &http_client::Url,
    client_id: &str,
    key: &[u8],
) -> Result<Value> {
    let data = crate::features::services::peers::management_http::query(
        state,
        base,
        client_id,
        key,
        json!({"query":"{ app { clientId deviceName deviceType httpsPort } }"}),
        65536,
    )
    .await
    .map_err(|error| {
        if error.downcast_ref::<http_client::Error>().is_some() {
            plain_libs::anyhow::anyhow!("failed_connect")
        } else {
            plain_libs::anyhow::anyhow!("invalid_session")
        }
    })?;
    ensure!(data["app"].is_object(), "invalid_session");
    Ok(data["app"].clone())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing_login.rs"]
mod tests;
