use plain_rs::features::services::auth_error::{AuthErrorCode, AuthFailure};
use crate::server::ServerState;
use plain_libs::serde_json::{Value, json};
use plain_rs::features::prefs::security::PasswordType;
use plain_rs::infrastructure::db::{AuditEventClient, SessionRow};
use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const LOGIN_ATTEMPT_LIMIT: usize = 5;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_TRACKED_CLIENTS: usize = 256;

pub(crate) fn login_error(error: &plain_libs::anyhow::Error) -> AuthErrorCode {
    let code = AuthErrorCode::for_failure(error);
    if code == AuthErrorCode::InternalError {
        log::error!("Login failed: {error:#}");
    }
    code
}

pub(crate) fn login_close_code(code: AuthErrorCode) -> u16 {
    match code {
        AuthErrorCode::TooManyLoginAttempts => 1013,
        AuthErrorCode::InternalError => 1011,
        _ => 1008,
    }
}

pub(crate) fn admit_attempt(
    state: &ServerState,
    client_id: &str,
    client_ip: &str,
) -> plain_libs::anyhow::Result<()> {
    plain_libs::anyhow::ensure!(
        plain_rs::features::client_identity::is_valid(client_id),
        AuthErrorCode::InvalidLoginRequest
    );
    let allowed = state
        .login_attempts
        .lock()
        .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?
        .acquire(&if client_ip.is_empty() {
            format!("cid:{client_id}")
        } else {
            client_ip.into()
        });
    plain_libs::anyhow::ensure!(allowed, AuthErrorCode::TooManyLoginAttempts);
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
            if let LoginProtection::Passwordless(channel) = &item.protection {
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
                    AuthFailure::LoginExpired
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
            .ok_or_else(|| plain_libs::anyhow::anyhow!(AuthErrorCode::InvalidPassword)),
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
            .ok_or_else(|| plain_libs::anyhow::anyhow!(AuthFailure::InvalidAuthResponse)),
            Self::Confirmation(c) => c.encrypt_response(bytes, now),
        }
    }
}

enum LoginProtection {
    Password {
        credential: plain_rs::features::prefs::security::PasswordCredential,
        channel: LoginChannel,
    },
    Passwordless(LoginChannel),
}

impl LoginProtection {
    fn channel(&self) -> &LoginChannel {
        match self {
            Self::Password { channel, .. } | Self::Passwordless(channel) => channel,
        }
    }

    fn validate(&self, state: &ServerState, request: &Value) -> plain_libs::anyhow::Result<()> {
        plain_libs::anyhow::ensure!(
            !plain_rs::features::prefs::security::password_setup_required(&state.prefs)?,
            AuthErrorCode::PasswordSetupRequired
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
                AuthErrorCode::InvalidPassword
            ),
            Self::Passwordless(_) => plain_libs::anyhow::ensure!(
                password_type == PasswordType::None && request.get("password").is_none(),
                AuthErrorCode::InvalidLoginRequest
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
                    LoginProtection::Passwordless(channel) => match channel {
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
        reason: AuthErrorCode,
    },
}

pub struct Confirmation {
    state: ServerState,
}

impl Confirmation {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }

    pub async fn decide(&self, request_id: &str, allowed: bool) -> Result<(), AuthErrorCode> {
        if allowed {
            if !self.state.main_ws.login_pending(request_id) {
                return Err(AuthErrorCode::Timeout);
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
                    let failure = login_error(&error);
                    self.state.main_ws.fail_login(request_id, failure);
                    return Err(failure);
                }
            };
            self.state
                .main_ws
                .complete(&self.state, request_id, &result)
                .await
                .map_err(|error| login_error(&error))
        } else {
            self.state.main_ws.reject(request_id);
            execute(
                &self.state,
                Request::Cancel {
                    request_id: request_id.into(),
                    reason: AuthErrorCode::Rejected,
                },
            )
            .await
            .map_err(|error| login_error(&error))?;
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

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoginMetadata {
    app_name: String,
    app_version: String,
    os_name: String,
    os_version: String,
    peer: Option<PeerMetadata>,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PeerMetadata {
    device_name: String,
    signature_public_key: String,
    encryption_public_key: String,
    port: u16,
    device_type: plain_rs::features::chat::enums::DeviceType,
    ips: Vec<String>,
}
fn validate_metadata(request: &Value) -> plain_libs::anyhow::Result<LoginMetadata> {
    let metadata: LoginMetadata = plain_libs::serde_json::from_value(request.clone())?;
    if let Some(peer) = &metadata.peer {
        plain_libs::anyhow::ensure!(
            peer.port != 0
                && peer
                    .ips
                    .iter()
                    .all(|ip| ip.parse::<std::net::IpAddr>().is_ok())
                && plain_libs::utils::base64::base64_decode_checked(&peer.signature_public_key)
                    .is_ok_and(|key| key.len() == 32)
                && plain_rs::features::peers::pairing::peer_store::encryption_key(&peer.encryption_public_key).is_ok(),
            AuthErrorCode::InvalidLoginRequest
        );
    }
    Ok(metadata)
}

fn peer_chat_paired(request: &Value) -> bool {
    request["peer"].is_object()
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
    let mut response =
        json!({"deviceId":device_client_id,"status":"COMPLETED","chatPaired":chat_paired});
    use plain_rs::features::peers::pairing::peer_store::{self, Facts};
    let peer = if chat_paired {
        let peer: PeerMetadata = plain_libs::serde_json::from_value(request.request["peer"].clone())?;
        let facts = Facts {
            id: request.client_id.clone(),
            name: peer.device_name,
            ip: request.client_ip.clone(),
            port: peer.port,
            device_type: peer.device_type,
            key: plain_rs::features::peers::pairing::security::login_chat_key(&shared),
            public_key: peer.signature_public_key,
            encryption_public_key: peer.encryption_public_key,
        };
        if let Some(saved) = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, &facts.id)? {
            facts.verify_peer(&saved).map_err(|_| AuthErrorCode::PeerIdentityChanged)?;
        }
        let encryption = plain_rs::features::prefs::security::encryption_public_key(&state.prefs)?;
        response["encryptionPublicKey"] = json!(plain_libs::utils::base64::base64_encode(&encryption));
        Some(facts)
    } else {
        None
    };
    let frame = request.protection.encrypt_response(&plain_libs::serde_json::to_vec(&response)?)?;
    let client = login_client(state, &request.client_id, &request.client_ip, &request.request)?;
    persist(state, request, &token)?;
    if let Some(facts) = peer {
        peer_store::save(&state.db, facts)?;
    }
    state
        .audit_events
        .record(
            plain_rs::features::audit_events::AuditEvent::Login(
                plain_rs::features::audit_events::LoginEventData {
                    session_type: plain_rs::features::sessions::SessionType::Login,
                },
            ),
            Some(client),
        )
        .map_err(plain_libs::anyhow::Error::msg)?;
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
        AuthErrorCode::InvalidLoginRequest
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
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| text(&request.request["peer"], "deviceName")),
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

fn login_client(
    state: &ServerState,
    client_id: &str,
    client_ip: &str,
    request: &Value,
) -> plain_libs::anyhow::Result<AuditEventClient> {
    let client_name = state
        .db
        .session_get(client_id)?
        .map(|row| row.name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| text(&request["peer"], "deviceName"));
    Ok(AuditEventClient {
        client_id: client_id.to_owned(),
        client_name,
        client_ip: client_ip.to_owned(),
        os_name: text(request, "osName"),
        os_version: text(request, "osVersion"),
        app_name: text(request, "appName"),
        app_version: text(request, "appVersion"),
    })
}

pub(crate) fn record_failure(
    state: &ServerState,
    client: &AuditEventClient,
    reason: AuthErrorCode,
) -> plain_libs::anyhow::Result<()> {
    state
        .audit_events
        .record(
            plain_rs::features::audit_events::AuditEvent::LoginFailed(
                plain_rs::features::audit_events::LoginFailedEventData {
                    reason,
                },
            ),
            Some(client.clone()),
        )
        .map_err(plain_libs::anyhow::Error::msg)?;
    Ok(())
}

pub(crate) async fn execute(
    state: &ServerState,
    request: Request,
) -> plain_libs::anyhow::Result<Value> {
    let mut client = match &request {
        Request::Issue {
            client_id,
            client_ip,
            ..
        } => Some(AuditEventClient {
            client_id: client_id.clone(),
            client_ip: client_ip.clone(),
            ..Default::default()
        }),
        Request::Complete { request_id } => {
            let pending = state
                .login_attempts
                .lock()
                .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?
                .pending
                .get(request_id)
                .map(|pending| {
                    (
                        pending.client_id.clone(),
                        pending.client_ip.clone(),
                        pending.request.clone(),
                    )
                });
            pending
                .map(|(id, ip, metadata)| login_client(state, &id, &ip, &metadata))
                .transpose()?
        }
        Request::Cancel { .. } => None,
    };
    let result = execute_request(state, request, &mut client).await;
    if let Err(error) = &result {
        if let Some(client) = client {
            record_failure(state, &client, login_error(error))?;
        }
    }
    result
}

async fn execute_request(
    state: &ServerState,
    request: Request,
    client: &mut Option<AuditEventClient>,
) -> plain_libs::anyhow::Result<Value> {
    if matches!(&request, Request::Issue { .. } | Request::Complete { .. }) {
        plain_libs::anyhow::ensure!(
            state.prefs.get_or("remote_access", true),
            AuthErrorCode::RemoteAccessDisabled
        );
    }
    let (pending, request_id) = match request {
        Request::Cancel { request_id, reason } => {
            let pending = {
                let mut attempts = state
                    .login_attempts
                    .lock()
                    .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?;
                let pending = attempts.pending.remove(&request_id);
                if pending.is_some() {
                    notify_closed(state, &mut attempts, &request_id);
                }
                pending
            };
            if let Some(pending) = pending {
                let client = login_client(
                    state,
                    &pending.client_id,
                    &pending.client_ip,
                    &pending.request,
                )?;
                record_failure(state, &client, reason)?;
            }
            return Ok(json!({"canceled":true}));
        }
        Request::Complete { request_id } => {
            let pending = {
                let mut attempts = state
                    .login_attempts
                    .lock()
                    .map_err(|_| plain_libs::anyhow::anyhow!("Login state unavailable"))?;
                let pending = attempts.pending.remove(&request_id).ok_or_else(|| {
                    plain_libs::anyhow::anyhow!(AuthFailure::LoginExpired)
                })?;
                notify_closed(state, &mut attempts, &request_id);
                pending
            };
            plain_libs::anyhow::ensure!(
                pending.at.elapsed() < Duration::from_secs(300),
                AuthFailure::LoginExpired
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
            *client = Some(login_client(state, &client_id, &client_ip, &request)?);
            let protection = match &channel {
                LoginChannel::Opaque(c) => {
                    plain_libs::anyhow::ensure!(
                        request.get("password").is_none(),
                        AuthErrorCode::InvalidLoginRequest
                    );
                    LoginProtection::Password {
                        credential: c.credential.clone(),
                        channel,
                    }
                }
                LoginChannel::Confirmation(_) => LoginProtection::Passwordless(channel),
            };
            request = plain_libs::serde_json::to_value(validate_metadata(&request)?)?;
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
            if pairing_requires_approval
                || state.prefs.get_or(
                    plain_rs::features::prefs::pref_keys::LOGIN_REQUIRE_APPROVAL,
                    true,
                )
            {
                let response = json!({"deviceId":state.prefs.get_or(plain_rs::features::prefs::pref_keys::CLIENT_ID,String::new()), "status":"PENDING"});
                let encrypted = pending
                    .protection
                    .encrypt_response(&plain_libs::serde_json::to_vec(&response)?)?;
                let mut request_facts = pending.request.clone();
                request_facts
                    .as_object_mut()
                    .unwrap()
                    .remove("verificationCode");
                if let LoginProtection::Passwordless(channel) = &pending.protection {
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
                    for canceled in attempts.cancel_client(&pending.client_id) {
                        notify_closed(state, &mut attempts, &canceled);
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
