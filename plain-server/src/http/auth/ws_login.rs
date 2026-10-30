use crate::assembly::server::ServerState;
use plain_rs::db::SessionRow;
use plain_rs::prefs::security::PasswordType;
use serde_json::{Value, json};
use sha2::{Digest, Sha256, Sha512};
use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const LOGIN_ATTEMPT_LIMIT: usize = 5;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_TRACKED_CLIENTS: usize = 256;

fn hash_to_token(hash: &str) -> Vec<u8> {
    hash.as_bytes().iter().copied().take(32).collect()
}

fn password_digest(prefs: &plain_rs::prefs::Prefs) -> String {
    let password = prefs
        .get::<String>(plain_rs::prefs::pref_keys::PASSWORD)
        .unwrap_or_default()
        .unwrap_or_default();
    plain_rs::utils::hex::bytes_to_hex(&Sha512::digest(password.as_bytes()))
}

pub(crate) fn admit_attempt(
    state: &ServerState,
    client_id: &str,
    client_ip: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(!client_id.is_empty(), "Client id is required");
    let allowed = state
        .login_attempts
        .lock()
        .map_err(|_| anyhow::anyhow!("Login state unavailable"))?
        .acquire(&if client_ip.is_empty() {
            format!("cid:{client_id}")
        } else {
            client_ip.into()
        });
    anyhow::ensure!(allowed, "too_many_login_attempts");
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or_default()
}

#[derive(Default)]
pub(crate) struct LoginAttempts {
    entries: HashMap<String, Vec<Instant>>,
    pending: HashMap<String, Pending>,
    recovery_pending: bool,
}

impl LoginAttempts {
    pub(crate) fn notify_host(&mut self, state: &ServerState, method: &'static str, params: Value) {
        if let Err(error) = state.host.notify(method, params) {
            log::warn!("Web login callback {method}: {error}");
            if self.recovery_pending || !state.host.connected() {
                return;
            }
            self.recovery_pending = true;
            let state = state.clone();
            tokio::spawn(async move {
                let permit = state.host.reserve_callback().await;
                let mut attempts = state.login_attempts.lock().unwrap();
                if let Ok(permit) = permit {
                    if let Err(error) = state.host.notify_reserved_callback(
                        "systemWebLoginSnapshot",
                        attempts.host_snapshot(),
                        permit,
                    ) {
                        log::warn!("Web login snapshot callback: {error}");
                    }
                }
                attempts.recovery_pending = false;
            });
        }
    }
    pub(crate) fn contains(&self, request_id: &str) -> bool {
        self.pending.contains_key(request_id)
    }

    pub(crate) fn host_snapshot(&self) -> Value {
        let mut pending: Vec<_> = self
            .pending
            .iter()
            .filter(|(_, item)| item.at.elapsed() < Duration::from_secs(300))
            .collect();
        pending.sort_by(|(left_id, left), (right_id, right)| {
            left.at.cmp(&right.at).then(left_id.cmp(right_id))
        });
        let requests: Vec<_> = pending.into_iter().map(|(id, item)| {
            let mut request = item.request.clone();
            request["password"] = json!("");
            request.as_object_mut().unwrap().remove("verificationCode");
            if let LoginProtection::DeviceConfirmation(channel) = &item.protection {
                request["verificationCode"] = json!(channel.verification_code);
            }
            json!({"requestId":id,"clientId":item.client_id,"clientIp":item.client_ip,"request":request})
        }).collect();
        json!({"requests":requests})
    }

    fn acquire(&mut self, key: &str) -> bool {
        let now = Instant::now();
        if !self.entries.contains_key(key) && self.entries.len() >= MAX_TRACKED_CLIENTS {
            self.entries.retain(|_, attempts| {
                attempts.retain(|at| now.duration_since(*at) < LOGIN_ATTEMPT_WINDOW);
                !attempts.is_empty()
            });
            if self.entries.len() >= MAX_TRACKED_CLIENTS {
                return false;
            }
        }
        let attempts = self.entries.entry(key.to_owned()).or_default();
        attempts.retain(|at| now.duration_since(*at) < LOGIN_ATTEMPT_WINDOW);
        if attempts.len() >= LOGIN_ATTEMPT_LIMIT {
            return false;
        }
        attempts.push(now);
        true
    }
}

struct Pending {
    client_id: String,
    client_ip: String,
    request: Value,
    at: Instant,
    protection: LoginProtection,
}

enum LoginProtection {
    Password {
        digest: String,
        channel: crate::services::confirmation_channel::Channel,
    },
    DeviceConfirmation(crate::services::confirmation_channel::Channel),
}

impl LoginProtection {
    fn channel(&self) -> &crate::services::confirmation_channel::Channel {
        match self {
            Self::Password { channel, .. } | Self::DeviceConfirmation(channel) => channel,
        }
    }

    fn validate(&self, state: &ServerState, request: &Value) -> anyhow::Result<()> {
        let channel = self.channel();
        channel.fresh(now_ms())?;
        anyhow::ensure!(
            text(request, "loginContext") == channel.context,
            "invalid_login_request"
        );

        let password_type = plain_rs::prefs::security::password_type(&state.prefs)?;
        match self {
            Self::Password { digest, .. } => anyhow::ensure!(
                password_type == PasswordType::Fixed
                    && *digest == password_digest(&state.prefs)
                    && text(request, "password") == *digest,
                "invalid_password"
            ),
            Self::DeviceConfirmation(_) => anyhow::ensure!(
                password_type == PasswordType::None && request.get("password").is_none(),
                "invalid_login_request"
            ),
        }
        Ok(())
    }

    fn decrypt_request(&self, frame: &[u8]) -> anyhow::Result<Vec<u8>> {
        let bytes = self.channel().decrypt_request(frame, now_ms())?;
        match self {
            Self::Password { digest, .. } => {
                plain_rs::crypto::xchacha_decrypt_raw(&hash_to_token(digest), &bytes)
                    .ok_or_else(|| anyhow::anyhow!("invalid_password"))
            }
            Self::DeviceConfirmation(_) => Ok(bytes),
        }
    }

    fn encrypt_response(&self, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
        match self {
            Self::Password { digest, channel } => {
                let encrypted =
                    plain_rs::crypto::xchacha_encrypt_raw(&hash_to_token(digest), bytes)
                        .ok_or_else(|| anyhow::anyhow!("Response encryption failed"))?;
                channel.encrypt_response(&encrypted, now_ms())
            }
            Self::DeviceConfirmation(channel) => channel.encrypt_response(bytes, now_ms()),
        }
    }
}

impl LoginAttempts {
    pub(crate) fn pending_requests(
        &self,
        after: Option<&str>,
    ) -> crate::native::management::NativeWebLoginRequests {
        use crate::native::management::{NativeWebLoginRequest, NativeWebLoginRequests};
        let mut entries = self
            .pending
            .iter()
            .filter(|(id, request)| {
                after.is_none_or(|after| id.as_str() > after)
                    && request.at.elapsed() < Duration::from_secs(300)
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        let next_after = (entries.len() > 64).then(|| entries[63].0.clone());
        let display = |value: &str| value.chars().take(128).collect::<String>();
        let requests = entries
            .into_iter()
            .take(64)
            .map(|(id, request)| NativeWebLoginRequest {
                request_id: id.clone(),
                client_id: display(&request.client_id),
                client_ip: display(&request.client_ip),
                browser_name: display(&text(&request.request, "browserName")),
                browser_version: display(&text(&request.request, "browserVersion")),
                os_name: display(&text(&request.request, "osName")),
                os_version: display(&text(&request.request, "osVersion")),
                verification_code: match &request.protection {
                    LoginProtection::DeviceConfirmation(channel) => {
                        Some(channel.verification_code.clone())
                    }
                    LoginProtection::Password { .. } => None,
                },
            })
            .collect();
        NativeWebLoginRequests {
            requests,
            next_after,
        }
    }

    pub(crate) fn cancel_client(&mut self, client_id: &str) -> Vec<String> {
        let removed = self
            .pending
            .iter()
            .filter(|(_, value)| value.client_id == client_id)
            .map(|(id, _)| id.clone())
            .collect();
        self.pending.retain(|_, value| value.client_id != client_id);
        removed
    }
}

pub(crate) enum Request {
    Issue {
        client_id: String,
        client_ip: String,
        frame: String,
        channel: crate::services::confirmation_channel::Channel,
    },
    Complete {
        request_id: String,
    },
    Cancel {
        request_id: String,
    },
}

pub struct Confirmation {
    state: ServerState,
}

impl Confirmation {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }

    pub async fn decide(&self, request_id: &str, allowed: bool) -> Result<(), String> {
        if allowed {
            if !self.state.main_ws.login_pending(request_id) {
                return Err("Login confirmation is no longer pending".into());
            }
            let result = execute(
                &self.state,
                Request::Complete {
                    request_id: request_id.into(),
                },
            )
            .await;
            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    let reason = match error.to_string().as_str() {
                        "login_expired" | "Login confirmation expired" => "login_expired",
                        "desktop_access_disabled" => "desktop_access_disabled",
                        "invalid_password" => "invalid_password",
                        _ => "invalid_login_request",
                    };
                    self.state.main_ws.fail_login(request_id, reason);
                    return Err(error.to_string());
                }
            };
            self.state
                .main_ws
                .complete(&self.state, request_id, &result)
                .await
                .map_err(|error| error.to_string())
        } else {
            self.state.main_ws.reject(request_id);
            execute(
                &self.state,
                Request::Cancel {
                    request_id: request_id.into(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
            Ok(())
        }
    }
}

pub(crate) fn notify_closed(state: &ServerState, attempts: &mut LoginAttempts, request_id: &str) {
    attempts.notify_host(
        state,
        "systemWebLoginClosed",
        json!({"requestId":request_id}),
    );
}

fn text(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn peer_chat_paired(request: &Value) -> bool {
    let Some(peer) = request.get("peer").filter(|value| !value.is_null()) else {
        return false;
    };
    let port = peer.get("port").and_then(Value::as_u64).unwrap_or(0);
    let key = text(peer, "signaturePublicKey");
    port >= 1 && port <= 65535 && plain_rs::utils::base64::base64_decode(&key).len() == 32
}

fn signature_data(
    device_client_id: &str,
    status: &str,
    ecdh_public_key: &str,
    timestamp: u64,
    chat_paired: bool,
) -> String {
    let paired = if chat_paired { "|true" } else { "" };
    format!("{device_client_id}|{status}|{ecdh_public_key}|{timestamp}{paired}")
}

fn sign(prefs: &plain_rs::prefs::Prefs, text: &str) -> anyhow::Result<String> {
    let keypair = crate::services::peers::peer_wire::signing_keypair(prefs)?;
    let signature = plain_rs::crypto::ed25519_sign(&keypair, text.as_bytes());
    anyhow::ensure!(
        !signature.is_empty(),
        "ed25519 signing produced no signature"
    );
    Ok(signature)
}

async fn issue(state: &ServerState, request: &Pending) -> anyhow::Result<Value> {
    let device_client_id = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .ok_or_else(|| anyhow::anyhow!("device client id is missing"))?;
    request.protection.validate(state, &request.request)?;
    let chat_paired = peer_chat_paired(&request.request);
    let keypair = plain_rs::crypto::EcdhSession::generate();
    let peer_public =
        plain_rs::utils::base64::base64_decode(&text(&request.request, "ecdhPublicKey"));
    let ecdh_public_key = plain_rs::utils::base64::base64_encode(&keypair.public_key_bytes);
    let shared = keypair
        .compute_shared_key(&peer_public)
        .ok_or_else(|| anyhow::anyhow!("ECDH shared key computation failed"))?;
    let token = plain_rs::utils::base64::base64_encode(&shared);
    let timestamp = now_ms();
    let mut signed = signature_data(
        &device_client_id,
        "COMPLETED",
        &ecdh_public_key,
        timestamp,
        chat_paired,
    );
    let channel = request.protection.channel();
    signed.push('|');
    signed.push_str(&channel.context);
    let signature = sign(&state.prefs, &signed)?;
    let mut response = json!({
        "clientId": device_client_id,
        "status": "COMPLETED",
        "ecdhPublicKey": ecdh_public_key,
        "signature": signature,
        "timestamp": timestamp,
        "chatPaired": chat_paired,
    });
    response["loginContext"] = json!(channel.context);
    if chat_paired {
        let id = &request.client_id;
        let offered_key = text(&request.request["peer"], "signaturePublicKey");
        if let Some(saved) = plain_rs::db::chat_store::peers::get(&state.db, id)? {
            anyhow::ensure!(
                saved.public_key.is_empty() || saved.public_key == offered_key,
                "Peer identity changed"
            );
        }
    }
    let frame = request
        .protection
        .encrypt_response(&serde_json::to_vec(&response)?)?;
    persist(state, request, &token)?;
    if chat_paired {
        let peer = &request.request["peer"];
        let mut material = b"plain-chat-pairing-v1".to_vec();
        material.extend(&shared);
        let mut ips = vec![request.client_ip.clone()];
        if let Some(extra) = peer["ips"].as_array() {
            ips.extend(extra.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        plain_rs::peers::pairing::peer_store::save(
            &state.db,
            plain_rs::peers::pairing::peer_store::Facts {
                id: request.client_id.clone(),
                name: text(peer, "deviceName"),
                ips,
                port: peer["port"].as_u64().unwrap() as u16,
                device_type: serde_json::from_value(peer["deviceType"].clone())?,
                key: plain_rs::utils::base64::base64_encode(&Sha256::digest(&material)),
                public_key: text(peer, "signaturePublicKey"),
            },
        )?;
    }
    Ok(
        json!({"status": "COMPLETED", "token": token, "response": response, "frame": plain_rs::utils::base64::base64_encode(&frame)}),
    )
}

fn persist(state: &ServerState, request: &Pending, token: &str) -> anyhow::Result<()> {
    let existing = state
        .db
        .session_get(&request.client_id)
        .map_err(|error| anyhow::Error::msg(error.to_string()))?;
    let now = plain_rs::utils::dbtime::now_iso();
    let row = SessionRow {
        client_id: request.client_id.clone(),
        name: existing
            .as_ref()
            .map(|row| row.name.clone())
            .unwrap_or_default(),
        r#type: existing
            .as_ref()
            .map(|row| row.r#type.clone())
            .unwrap_or_else(|| "WEB".into()),
        client_ip: request.client_ip.clone(),
        os_name: text(&request.request, "osName"),
        os_version: text(&request.request, "osVersion"),
        browser_name: text(&request.request, "browserName"),
        browser_version: text(&request.request, "browserVersion"),
        token: token.to_owned(),
        last_seen_at: existing
            .as_ref()
            .and_then(|row| row.last_seen_at.clone())
            .or_else(|| Some(now.clone())),
        created_at: existing
            .as_ref()
            .map(|row| row.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    state
        .db
        .session_save(&row)
        .map_err(|error| anyhow::Error::msg(error.to_string()))?;
    Ok(())
}

pub(crate) async fn execute(state: &ServerState, request: Request) -> anyhow::Result<Value> {
    if matches!(&request, Request::Issue { .. } | Request::Complete { .. }) {
        anyhow::ensure!(
            state.prefs.get_or("desktop_access", true),
            "desktop_access_disabled"
        );
    }
    let (pending, request_id) = match request {
        Request::Cancel { request_id } => {
            let mut attempts = state
                .login_attempts
                .lock()
                .map_err(|_| anyhow::anyhow!("Login state unavailable"))?;
            if attempts.pending.remove(&request_id).is_some() {
                notify_closed(state, &mut attempts, &request_id);
            }
            return Ok(json!({"cancelled":true}));
        }
        Request::Complete { request_id } => {
            let pending = {
                let mut attempts = state
                    .login_attempts
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Login state unavailable"))?;
                let pending = attempts
                    .pending
                    .remove(&request_id)
                    .ok_or_else(|| anyhow::anyhow!("Login confirmation is no longer pending"))?;
                notify_closed(state, &mut attempts, &request_id);
                pending
            };
            anyhow::ensure!(
                pending.at.elapsed() < Duration::from_secs(300),
                "Login confirmation expired"
            );
            (pending, request_id)
        }
        Request::Issue {
            client_id,
            client_ip,
            frame,
            channel,
        } => {
            let protection = match channel.mode {
                crate::services::confirmation_channel::ConfirmationMode::Password => {
                    LoginProtection::Password {
                        digest: password_digest(&state.prefs),
                        channel,
                    }
                }
                crate::services::confirmation_channel::ConfirmationMode::ChatPairing => {
                    anyhow::bail!("Invalid login confirmation mode")
                }
                crate::services::confirmation_channel::ConfirmationMode::DeviceConfirmation => {
                    LoginProtection::DeviceConfirmation(channel)
                }
            };
            let frame = plain_rs::utils::base64::base64_decode(&frame);
            let bytes = protection.decrypt_request(&frame)?;
            let request: Value = serde_json::from_slice(&bytes)?;
            for field in ["browserName", "browserVersion", "osName", "osVersion"] {
                anyhow::ensure!(
                    request[field].is_string(),
                    "Invalid login metadata: {field}"
                );
            }
            anyhow::ensure!(
                request["isMobile"].is_boolean(),
                "Invalid login metadata: isMobile"
            );
            if let Some(peer) = request.get("peer").filter(|peer| !peer.is_null()) {
                anyhow::ensure!(
                    peer["deviceName"].is_string()
                        && peer["signaturePublicKey"].is_string()
                        && peer["port"]
                            .as_i64()
                            .is_some_and(|port| i32::try_from(port).is_ok()),
                    "Invalid peer metadata"
                );
                let _: plain_rs::chat::enums::DeviceType =
                    serde_json::from_value(peer["deviceType"].clone())?;
                let _: Vec<String> = serde_json::from_value(peer["ips"].clone())?;
            }

            protection.validate(state, &request)?;
            let pending = Pending {
                client_id,
                client_ip,
                request,
                at: Instant::now(),
                protection,
            };
            let id = uuid::Uuid::new_v4().to_string();
            if peer_chat_paired(&pending.request)
                || state
                    .prefs
                    .get_or(plain_rs::prefs::pref_keys::LOGIN_REQUIRE_APPROVAL, true)
            {
                let response = json!({"clientId":state.prefs.get_or(plain_rs::prefs::pref_keys::CLIENT_ID,String::new()), "status":"PENDING"});
                let encrypted = pending
                    .protection
                    .encrypt_response(&serde_json::to_vec(&response)?)?;
                let mut request_facts = pending.request.clone();
                request_facts
                    .as_object_mut()
                    .unwrap()
                    .remove("verificationCode");
                if let LoginProtection::DeviceConfirmation(channel) = &pending.protection {
                    request_facts["verificationCode"] = json!(channel.verification_code);
                }
                {
                    let mut attempts = state
                        .login_attempts
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Login state unavailable"))?;
                    attempts
                        .pending
                        .retain(|_, item| item.at.elapsed() < Duration::from_secs(300));
                    for cancelled in attempts.cancel_client(&pending.client_id) {
                        notify_closed(state, &mut attempts, &cancelled);
                    }
                    attempts.pending.insert(id.clone(), pending);
                }
                return Ok(
                    json!({"status":"PENDING","requestId":id,"request":request_facts,"frame":plain_rs::utils::base64::base64_encode(&encrypted)}),
                );
            }
            (pending, id)
        }
    };
    let mut result = issue(state, &pending).await?;
    result["requestId"] = json!(request_id);
    result["request"] = pending.request;
    result["clientIp"] = json!(pending.client_ip);
    Ok(result)
}

#[cfg(test)]
#[path = "../../../tests/unit/http/auth/ws_login.rs"]
mod tests;
