use crate::server::ServerState;
use plain_rs::features::prefs::security::PasswordType;
use plain_rs::infrastructure::db::SessionRow;
use plain_libs::serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const LOGIN_ATTEMPT_LIMIT: usize = 5;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_TRACKED_CLIENTS: usize = 256;

pub(crate) fn admit_attempt(
    state: &ServerState,
    client_id: &str,
    client_ip: &str,
) -> plain_libs::anyhow::Result<()> {
    plain_libs::anyhow::ensure!(!client_id.is_empty(), "Client id is required");
    let allowed = state
        .login_attempts
        .lock()
        .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?
        .acquire(&if client_ip.is_empty() {
            format!("cid:{client_id}")
        } else {
            client_ip.into()
        });
    plain_libs::anyhow::ensure!(allowed, "too_many_login_attempts");
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
    pub(crate) fn notify_host(
        &mut self,
        host: std::sync::Arc<plain_rs::infrastructure::host::bridge::Host>,
        login_attempts: std::sync::Arc<std::sync::Mutex<LoginAttempts>>,
        method: &'static str,
        params: Value,
    ) {
        if let Err(error) = host.notify(method, params) {
            log::warn!("Web login callback {method}: {error}");
            if self.recovery_pending || !host.connected() {
                return;
            }
            self.recovery_pending = true;
            tokio::spawn(async move {
                let permit = host.reserve_callback().await;
                let mut attempts = login_attempts.lock().unwrap();
                if let Ok(permit) = permit {
                    if let Err(error) = host.notify_reserved_callback(
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
                request["verificationCode"] = json!(match channel { LoginChannel::Confirmation(c) => c.verification_code.as_str(), _ => "" });
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

pub(crate) struct OpaqueChannel {
    pub key: [u8; 32],
    pub context: Vec<u8>,
    pub credential: plain_rs::features::prefs::security::PasswordCredential,
    pub created: Instant,
}
pub(crate) enum LoginChannel {
    Opaque(OpaqueChannel),
    Confirmation(plain_rs::features::services::confirmation_channel::Channel),
}
impl LoginChannel {
    #[cfg(test)]
    fn context(&self) -> String {
        match self {
            Self::Opaque(c) => plain_libs::utils::base64::base64_encode(&c.context),
            Self::Confirmation(c) => c.context.clone(),
        }
    }
    fn fresh(&self, now: u64) -> plain_libs::anyhow::Result<()> {
        match self {
            Self::Opaque(c) => {
                plain_libs::anyhow::ensure!(
                    c.created.elapsed() < Duration::from_secs(300),
                    "login_expired"
                );
                Ok(())
            }
            Self::Confirmation(c) => c.fresh(now),
        }
    }
    fn decrypt_request(&self, bytes: &[u8], now: u64) -> plain_libs::anyhow::Result<Vec<u8>> {
        self.fresh(now)?;
        match self {
            Self::Opaque(c) => plain_libs::crypto::xchacha_decrypt_raw_with_aad(
                &c.key,
                bytes,
                &plain_libs::auth::auth_aad(&c.context, false),
            )
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_password")),
            Self::Confirmation(c) => c.decrypt_request(bytes, now),
        }
    }
    fn encrypt_response(&self, bytes: &[u8], now: u64) -> plain_libs::anyhow::Result<Vec<u8>> {
        self.fresh(now)?;
        match self {
            Self::Opaque(c) => plain_libs::crypto::xchacha_encrypt_raw_with_aad(
                &c.key,
                bytes,
                &plain_libs::auth::auth_aad(&c.context, true),
            )
            .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid_auth_response")),
            Self::Confirmation(c) => c.encrypt_response(bytes, now),
        }
    }
}

enum LoginProtection {
    Password {
        credential: plain_rs::features::prefs::security::PasswordCredential,
        channel: LoginChannel,
    },
    DeviceConfirmation(LoginChannel),
}

impl LoginProtection {
    fn channel(&self) -> &LoginChannel {
        match self {
            Self::Password { channel, .. } | Self::DeviceConfirmation(channel) => channel,
        }
    }

    fn validate(&self, state: &ServerState, request: &Value) -> plain_libs::anyhow::Result<()> {
        plain_libs::anyhow::ensure!(
            !plain_rs::features::prefs::security::password_setup_required(&state.prefs)?,
            "password_setup_required"
        );
        let channel = self.channel();
        channel.fresh(now_ms())?;

        let password_type = plain_rs::features::prefs::security::password_type(&state.prefs)?;
        match self {
            Self::Password { credential, .. } => plain_libs::anyhow::ensure!(
                password_type == PasswordType::Fixed
                    && Some(credential)
                        == plain_rs::features::prefs::security::password_credential(&state.prefs)?
                            .as_ref(),
                "invalid_password"
            ),
            Self::DeviceConfirmation(_) => plain_libs::anyhow::ensure!(
                password_type == PasswordType::None && request.get("password").is_none(),
                "invalid_login_request"
            ),
        }
        Ok(())
    }

    fn encrypt_response(&self, bytes: &[u8]) -> plain_libs::anyhow::Result<Vec<u8>> {
        self.channel().encrypt_response(bytes, now_ms())
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
                app_name: display(&text(&request.request, "appName")),
                app_version: display(&text(&request.request, "appVersion")),
                os_name: display(&text(&request.request, "osName")),
                os_version: display(&text(&request.request, "osVersion")),
                verification_code: match &request.protection {
                    LoginProtection::DeviceConfirmation(channel) => match channel {
                        LoginChannel::Confirmation(c) => Some(c.verification_code.clone()),
                        _ => None,
                    },
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
        channel: LoginChannel,
    },
    Complete {
        request_id: String,
    },
    Cancel {
        request_id: String,
        reason: &'static str,
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
                        "remote_access_disabled" => "remote_access_disabled",
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
                    reason: "rejected",
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
        state.host.clone(),
        state.login_attempts.clone(),
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
    port >= 1 && port <= 65535 && plain_libs::utils::base64::base64_decode(&key).len() == 32
}

async fn issue(state: &ServerState, request: &Pending) -> plain_libs::anyhow::Result<Value> {
    let device_client_id = state
        .prefs
        .get::<String>(plain_rs::features::prefs::pref_keys::CLIENT_ID)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("device client id is missing"))?;
    request.protection.validate(state, &request.request)?;
    let chat_paired = peer_chat_paired(&request.request);
    let shared = match request.protection.channel() {
        LoginChannel::Opaque(c) => c.key.to_vec(),
        LoginChannel::Confirmation(c) => c.api_key().to_vec(),
    };
    let token = plain_libs::utils::base64::base64_encode(&shared);
    let response =
        json!({"clientId":device_client_id,"status":"COMPLETED","chatPaired":chat_paired});
    if chat_paired {
        let id = &request.client_id;
        let offered_key = text(&request.request["peer"], "signaturePublicKey");
        if let Some(saved) = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, id)? {
            plain_libs::anyhow::ensure!(
                saved.public_key.is_empty() || saved.public_key == offered_key,
                "Peer identity changed"
            );
        }
    }
    let frame = request
        .protection
        .encrypt_response(&plain_libs::serde_json::to_vec(&response)?)?;
    persist(state, request, &token)?;
    if chat_paired {
        let peer = &request.request["peer"];
        let mut material = b"chat-pairing".to_vec();
        material.extend(&shared);
        let mut ips = vec![request.client_ip.clone()];
        if let Some(extra) = peer["ips"].as_array() {
            ips.extend(extra.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        plain_rs::features::peers::pairing::peer_store::save(
            &state.db,
            plain_rs::features::peers::pairing::peer_store::Facts {
                id: request.client_id.clone(),
                name: text(peer, "deviceName"),
                ips,
                port: peer["port"].as_u64().unwrap() as u16,
                device_type: plain_libs::serde_json::from_value(peer["deviceType"].clone())?,
                key: plain_libs::utils::base64::base64_encode(&Sha256::digest(&material)),
                public_key: text(peer, "signaturePublicKey"),
            },
        )?;
    }
    Ok(
        json!({"status": "COMPLETED", "token": token, "response": response, "frame": plain_libs::utils::base64::base64_encode(&frame)}),
    )
}

fn persist(state: &ServerState, request: &Pending, token: &str) -> plain_libs::anyhow::Result<()> {
    plain_libs::anyhow::ensure!(
        request.client_id
            != state
                .native_client_id()
                .map_err(plain_libs::anyhow::Error::msg)?,
        "invalid_login_request"
    );
    let existing = state
        .db
        .session_get(&request.client_id)
        .map_err(|error| plain_libs::anyhow::Error::msg(error.to_string()))?;
    let now = plain_libs::utils::dbtime::now_iso();
    let row = SessionRow {
        url_token: plain_libs::crypto::gen_token(),
        client_id: request.client_id.clone(),
        name: existing
            .as_ref()
            .map(|row| row.name.clone())
            .unwrap_or_default(),
        r#type: existing
            .as_ref()
            .map(|row| row.r#type)
            .unwrap_or(plain_rs::features::sessions::SessionType::Login),
        client_ip: request.client_ip.clone(),
        os_name: text(&request.request, "osName"),
        os_version: text(&request.request, "osVersion"),
        app_name: text(&request.request, "appName"),
        app_version: text(&request.request, "appVersion"),
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
        .map_err(|error| plain_libs::anyhow::Error::msg(error.to_string()))?;
    Ok(())
}

pub(crate) fn record_failure(state: &ServerState, client_id: &str, reason: &str) -> plain_libs::anyhow::Result<()> {
    state.db.audit_event_add("login_failed", reason, Some(client_id))?;
    Ok(())
}

pub(crate) async fn execute(state: &ServerState, request: Request) -> plain_libs::anyhow::Result<Value> {
    let client = match &request {
        Request::Issue { client_id, .. } => Some(client_id.clone()),
        Request::Complete { request_id } => state.login_attempts.lock()
            .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?
            .pending.get(request_id).map(|pending| pending.client_id.clone()),
        Request::Cancel { .. } => None,
    };
    let result = execute_request(state, request).await;
    match &result {
        Ok(value) if value["status"] == "COMPLETED" => {
            state.db.audit_event_add("login", &text(&value["request"], "appName"), client.as_deref())?;
        }
        Err(error) => if let Some(client) = client {
            let reason = match error.to_string().as_str() {
                "invalid_password" => "invalid_password",
                "remote_access_disabled" => "remote_access_disabled",
                "Login confirmation expired" | "login_expired" => "login_expired",
                "too_many_login_attempts" => "too_many_login_attempts",
                _ => "invalid_login_request",
            };
            record_failure(state, &client, reason)?;
        },
        _ => {}
    }
    result
}

async fn execute_request(state: &ServerState, request: Request) -> plain_libs::anyhow::Result<Value> {
    if matches!(&request, Request::Issue { .. } | Request::Complete { .. }) {
        plain_libs::anyhow::ensure!(
            state.prefs.get_or("remote_access", true),
            "remote_access_disabled"
        );
    }
    let (pending, request_id) = match request {
        Request::Cancel { request_id, reason } => {
            let pending = {
                let mut attempts = state.login_attempts.lock()
                    .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?;
                let pending = attempts.pending.remove(&request_id);
                if pending.is_some() { notify_closed(state, &mut attempts, &request_id); }
                pending
            };
            if let Some(pending) = pending {
                record_failure(state, &pending.client_id, reason)?;
            }
            return Ok(json!({"cancelled":true}));
        }
        Request::Complete { request_id } => {
            let pending = {
                let mut attempts = state
                    .login_attempts
                    .lock()
                    .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?;
                let pending = attempts
                    .pending
                    .remove(&request_id)
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Login confirmation is no longer pending"))?;
                notify_closed(state, &mut attempts, &request_id);
                pending
            };
            plain_libs::anyhow::ensure!(
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
            let frame = plain_libs::utils::base64::base64_decode_checked(&frame)?;
            let bytes = channel.decrypt_request(&frame, now_ms())?;
            let mut request: Value = plain_libs::serde_json::from_slice(&bytes)?;
            let protection = match &channel {
                LoginChannel::Opaque(c) => {
                    plain_libs::anyhow::ensure!(request.get("password").is_none(), "invalid_login_request");
                    LoginProtection::Password {
                        credential: c.credential.clone(),
                        channel,
                    }
                }
                LoginChannel::Confirmation(_) => LoginProtection::DeviceConfirmation(channel),
            };
            for field in ["appName", "appVersion", "osName", "osVersion"] {
                plain_libs::anyhow::ensure!(
                    request[field].is_string(),
                    "Invalid login metadata: {field}"
                );
            }
            let os_name = text(&request, "osName");
            let canonical_os_name = match os_name.trim().to_ascii_lowercase().as_str() {
                "darwin" | "mac" | "macos" | "mac os x" => "macOS",
                "window" | "windows" | "win32" => "Windows",
                "linux" => "Linux",
                "android" => "Android",
                "ios" => "iOS",
                "ipados" => "iPadOS",
                _ => os_name.as_str(),
            };
            request["osName"] = json!(canonical_os_name);
            plain_libs::anyhow::ensure!(
                request["isMobile"].is_boolean(),
                "Invalid login metadata: isMobile"
            );
            if let Some(peer) = request.get("peer").filter(|peer| !peer.is_null()) {
                plain_libs::anyhow::ensure!(
                    peer["deviceName"].is_string()
                        && peer["signaturePublicKey"].is_string()
                        && peer["port"]
                            .as_i64()
                            .is_some_and(|port| i32::try_from(port).is_ok()),
                    "Invalid peer metadata"
                );
                let _: plain_rs::features::chat::enums::DeviceType =
                    plain_libs::serde_json::from_value(peer["deviceType"].clone())?;
                let _: Vec<String> = plain_libs::serde_json::from_value(peer["ips"].clone())?;
            }

            protection.validate(state, &request)?;
            let pending = Pending {
                client_id,
                client_ip,
                request,
                at: Instant::now(),
                protection,
            };
            let id = plain_libs::utils::uuid::Uuid::new_v4().into_string();
            let pairing_requires_approval = peer_chat_paired(&pending.request)
                && (state.pairing_approval != crate::server::PairingApproval::FollowPasswordLogin
                    || !matches!(&pending.protection, LoginProtection::Password { .. }));
            if matches!(&pending.protection, LoginProtection::DeviceConfirmation(_))
                || pairing_requires_approval
                || state.prefs.get_or(
                    plain_rs::features::prefs::pref_keys::LOGIN_REQUIRE_APPROVAL,
                    true,
                )
            {
                let response = json!({"clientId":state.prefs.get_or(plain_rs::features::prefs::pref_keys::CLIENT_ID,String::new()), "status":"PENDING"});
                let encrypted = pending
                    .protection
                    .encrypt_response(&plain_libs::serde_json::to_vec(&response)?)?;
                let mut request_facts = pending.request.clone();
                request_facts
                    .as_object_mut()
                    .unwrap()
                    .remove("verificationCode");
                if let LoginProtection::DeviceConfirmation(channel) = &pending.protection {
                    request_facts["verificationCode"] = json!(match channel {
                        LoginChannel::Confirmation(c) => c.verification_code.as_str(),
                        _ => "",
                    });
                }
                {
                    let mut attempts = state
                        .login_attempts
                        .lock()
                        .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?;
                    attempts
                        .pending
                        .retain(|_, item| item.at.elapsed() < Duration::from_secs(300));
                    for cancelled in attempts.cancel_client(&pending.client_id) {
                        notify_closed(state, &mut attempts, &cancelled);
                    }
                    attempts.pending.insert(id.clone(), pending);
                }
                return Ok(
                    json!({"status":"PENDING","requestId":id,"request":request_facts,"frame":plain_libs::utils::base64::base64_encode(&encrypted)}),
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
#[path = "../../../../tests/plain-server/unit/http/auth/ws_login.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/auth/opaque_channel.rs"]
mod opaque_tests;
