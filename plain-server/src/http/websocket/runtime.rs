use crate::assembly::server::ServerState;
use crate::services::confirmation_channel;
use axum::{
    extract::{
        WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use plain_rs::events::WsEvent;
use plain_rs::prefs::security::PasswordType;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc;

const MAX_UNAUTHENTICATED_CONNECTIONS: usize = 128;
const MAX_UNAUTHENTICATED_CONNECTIONS_PER_IP: usize = 5;
const INITIAL_MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Runtime {
    pub(crate) screen_events: tokio::sync::broadcast::Sender<WsEvent>,
    connections: Mutex<HashMap<String, Connection>>,
    unauthenticated: Mutex<HashMap<IpAddr, usize>>,
    replay: crate::http::auth::replay::Replay,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            connections: Mutex::default(),
            unauthenticated: Mutex::default(),
            replay: Default::default(),
            screen_events: tokio::sync::broadcast::channel(256).0,
        }
    }
}

struct UnauthenticatedConnection {
    runtime: Arc<Runtime>,
    ip: IpAddr,
}

impl Drop for UnauthenticatedConnection {
    fn drop(&mut self) {
        let mut active = self.runtime.unauthenticated.lock().unwrap();
        let count = active.get_mut(&self.ip).unwrap();
        *count -= 1;
        if *count == 0 {
            active.remove(&self.ip);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Channel {
    Auth,
    Screen,
    ImageEditor,
}

pub(crate) fn is_screen_event(kind: &str) -> bool {
    matches!(kind, "SCREEN_MIRROR_VIDEO" | "SCREEN_MIRROR_AUDIO")
}

struct Connection {
    channel: Channel,
    client_id: String,
    registered: bool,
    outgoing: mpsc::Sender<Message>,
    pending: Option<String>,
    closing: tokio::sync::watch::Sender<Option<(u16, &'static str)>>,
}
impl Runtime {
    fn reserve_unauthenticated(self: &Arc<Self>, ip: IpAddr) -> Option<UnauthenticatedConnection> {
        let mut active = self.unauthenticated.lock().unwrap();
        if active.get(&ip).copied().unwrap_or(0) >= MAX_UNAUTHENTICATED_CONNECTIONS_PER_IP
            || active.values().sum::<usize>() >= MAX_UNAUTHENTICATED_CONNECTIONS
        {
            return None;
        }
        *active.entry(ip).or_default() += 1;
        Some(UnauthenticatedConnection {
            runtime: self.clone(),
            ip,
        })
    }

    pub(crate) fn login_pending(&self, request_id: &str) -> bool {
        self.connections.lock().unwrap().values().any(|connection| {
            connection.pending.as_deref() == Some(request_id)
                && connection.closing.borrow().is_none()
        })
    }
    pub(crate) fn close_client(&self, state: &ServerState, client_id: Option<&str>) {
        state.subscription_sessions.close_client(client_id);
        let connections = self.connections.lock().unwrap();
        for c in connections
            .values()
            .filter(|c| client_id.is_none_or(|id| c.client_id == id))
        {
            let _ = c.closing.send(Some((1000, "")));
        }
        drop(connections);
    }
    pub(crate) async fn complete(
        &self,
        state: &ServerState,
        request_id: &str,
        result: &Value,
    ) -> anyhow::Result<()> {
        let outgoing = self
            .connections
            .lock()
            .unwrap()
            .iter_mut()
            .find(|(_, c)| c.pending.as_deref() == Some(request_id))
            .map(|(_, c)| {
                c.pending = None;
                c.outgoing.clone()
            });
        if let Some(outgoing) = outgoing {
            tokio::time::timeout(
                Duration::from_secs(2),
                outgoing.send(Message::Binary(plain_rs::utils::base64::base64_decode(
                    result["frame"].as_str().unwrap_or_default(),
                ))),
            )
            .await??;
            completed(state, result).await;
        }
        Ok(())
    }
    pub(crate) fn reject(&self, request_id: &str) {
        self.fail_login(request_id, "rejected");
    }
    pub(crate) fn fail_login(&self, request_id: &str, reason: &'static str) {
        if let Some(c) = self
            .connections
            .lock()
            .unwrap()
            .values()
            .find(|c| c.pending.as_deref() == Some(request_id))
        {
            let _ = c.closing.send(Some((1013, reason)));
        }
    }
}
fn close(code: u16, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}
fn enabled(state: &ServerState) -> bool {
    state.prefs.get_or("desktop_access", true)
}
#[derive(Deserialize)]
pub(crate) struct Params {
    #[serde(default)]
    cid: String,
    auth: Option<String>,
}
pub(super) fn upgrade(
    state: ServerState,
    params: Params,
    remote: SocketAddr,
    ws: WebSocketUpgrade,
    channel: Channel,
) -> Response {
    let Some(permit) = state.main_ws.reserve_unauthenticated(remote.ip()) else {
        return (StatusCode::TOO_MANY_REQUESTS, "too_many_login_connections").into_response();
    };
    ws.max_message_size(65536)
        .max_frame_size(65536)
        .on_upgrade(move |socket| {
            run(
                socket,
                state,
                params,
                remote.ip().to_string(),
                channel,
                permit,
            )
        })
        .into_response()
}

async fn completed(state: &ServerState, result: &Value) {
    let mut request = result["request"].clone();
    if let Some(request) = request.as_object_mut() {
        request.remove("password");
    }
    if let Err(error) = state.host.notify(
        "systemWebLoginCompleted",
        json!({"request":request,"chatPaired":result["response"]["chatPaired"],"clientIp":result["clientIp"]}),
    ) {
        log::warn!("Web login completion callback: {error}");
    }
}
async fn run(
    mut socket: WebSocket,
    mut state: ServerState,
    params: Params,
    ip: String,
    channel: Channel,
    permit: UnauthenticatedConnection,
) {
    let mut permit = Some(permit);
    if (channel == Channel::Auth && !matches!(params.auth.as_deref(), Some("1" | "2")))
        || (channel != Channel::Auth && params.auth.is_some())
    {
        let _ = socket.send(close(1008, "invalid_request")).await;
        return;
    }
    if !enabled(&state) || params.cid.is_empty() {
        let reason = if params.cid.is_empty() {
            "`cid` is missing"
        } else {
            "desktop_access_disabled"
        };
        let _ = socket.send(close(1008, reason)).await;
        return;
    }
    if channel == Channel::Auth {
        if let Err(error) = crate::http::auth::ws_login::admit_attempt(&state, &params.cid, &ip) {
            let reason = if error.to_string() == "too_many_login_attempts" {
                "too_many_login_attempts"
            } else {
                "invalid_login_request"
            };
            let _ = socket.send(close(1013, reason)).await;
            return;
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let (sender, mut outgoing) = mpsc::channel(8);
    let (closing, mut closed) = tokio::sync::watch::channel(None);
    state.main_ws.connections.lock().unwrap().insert(
        id.clone(),
        Connection {
            channel,
            client_id: params.cid.clone(),
            registered: false,
            outgoing: sender,
            pending: None,
            closing,
        },
    );
    let mut registered = false;
    let mut stream = None;
    let mut receiver = match channel {
        Channel::Auth | Channel::ImageEditor => state.events.subscribe(),
        Channel::Screen => state.main_ws.screen_events.subscribe(),
    };
    let mut pending: Option<String> = None;
    enum LoginHandshake {
        AwaitingCommitment,
        AwaitingReveal(confirmation_channel::ServerHandshake),
        Ready(confirmation_channel::Channel),
        Submitted,
    }
    let mut handshake = LoginHandshake::AwaitingCommitment;
    let initial_deadline = tokio::time::sleep(INITIAL_MESSAGE_TIMEOUT);
    tokio::pin!(initial_deadline);
    let deadline = tokio::time::sleep(Duration::from_secs(300));
    tokio::pin!(deadline);
    loop {
        let message = tokio::select! {
            _ = state.stop.changed() => break,
            _ = &mut initial_deadline, if !registered && !matches!(handshake, LoginHandshake::Submitted) => {
                let _ = socket.send(close(1008, "timeout")).await;
                break;
            },
            _ = &mut deadline, if matches!(params.auth.as_deref(), Some("1" | "2")) => { let _ = socket.send(close(1008, "login_expired")).await; break; },
            _ = closed.changed() => { let (code, reason) = (*closed.borrow()).unwrap_or((1000, "")); let _ = socket.send(close(code, reason)).await; break; },
            message = outgoing.recv() => match message { Some(message) => message, None => break },
            incoming = socket.next() => {
                let frame = match incoming {
                    Some(Ok(Message::Binary(frame))) => frame,
                    Some(Ok(Message::Ping(bytes))) => { if socket.send(Message::Pong(bytes)).await.is_err() { break; } continue; },
                    Some(Ok(Message::Text(text))) if matches!(params.auth.as_deref(), Some("1" | "2")) => {
                        if text.len() > 4096 {
                            let _ = socket.send(close(1008, "invalid_login_request")).await;
                            break;
                        }
                        let accepted = (|| -> anyhow::Result<_> {
                            match std::mem::replace(&mut handshake, LoginHandshake::Submitted) {
                                LoginHandshake::AwaitingCommitment => {
                                    let password_type = plain_rs::prefs::security::password_type(&state.prefs)?;
                                    let mode = match (params.auth.as_deref(), password_type) {
                                        (Some("1"), PasswordType::Fixed) => confirmation_channel::ConfirmationMode::Password,
                                        (Some("2"), PasswordType::None) => confirmation_channel::ConfirmationMode::DeviceConfirmation,
                                        _ => anyhow::bail!("invalid_login_request"),
                                    };
                                    anyhow::ensure!(enabled(&state), "desktop_access_disabled");
                                    let request = serde_json::from_str::<confirmation_channel::ClientHello>(&text)?;
                                    let device_id = state.prefs.get_or(
                                        plain_rs::prefs::pref_keys::CLIENT_ID,
                                        String::new(),
                                    );
                                    let signing = crate::services::peers::peer_wire::signing_keypair(&state.prefs)?;
                                    let now = chrono::Utc::now().timestamp_millis() as u64;
                                    let (pending, response) = confirmation_channel::ServerHandshake::start(
                                        &params.cid,
                                        mode,
                                        &device_id,
                                        &request,
                                        &signing,
                                        now,
                                    )?;
                                    handshake = LoginHandshake::AwaitingReveal(pending);
                                    Ok(Some(response))
                                }
                                LoginHandshake::AwaitingReveal(pending) => {
                                    let reveal = serde_json::from_str::<confirmation_channel::ClientReveal>(&text)?;
                                    let now = chrono::Utc::now().timestamp_millis() as u64;
                                    handshake = LoginHandshake::Ready(pending.finish(&reveal, now)?);
                                    Ok(None)
                                }
                                _ => anyhow::bail!("invalid_login_request"),
                            }
                        })();
                        match accepted {
                            Ok(Some(response)) => { if socket.send(Message::Text(serde_json::to_string(&response).unwrap())).await.is_err() { break; } }
                            Ok(None) => {}
                            Err(error) => {
                                let reason = if error.to_string() == "too_many_login_attempts" { "too_many_login_attempts" } else { "invalid_login_request" };
                                let _ = socket.send(close(1008, reason)).await;
                                break;
                            }
                        }
                        continue;
                    }
                    Some(Ok(Message::Text(_) | Message::Pong(_))) => continue,
                    _ => break,
                };
                if !enabled(&state) { let _ = socket.send(close(1008,"desktop_access_disabled")).await; break; }
                if matches!(params.auth.as_deref(), Some("1" | "2")) {
                    if let Some(request_id) = pending.take() { let _ = crate::http::auth::ws_login::execute(&state, crate::http::auth::ws_login::Request::Cancel { request_id }).await; }
                    let LoginHandshake::Ready(channel) = std::mem::replace(&mut handshake, LoginHandshake::Submitted) else {
                        let _ = socket.send(close(1008, "invalid_login_request")).await;
                        break;
                    };
                    let request = crate::http::auth::ws_login::Request::Issue {
                        client_id: params.cid.clone(),
                        client_ip: ip.clone(),
                        frame: plain_rs::utils::base64::base64_encode(&frame),
                        channel,
                    };
                    match crate::http::auth::ws_login::execute(&state, request).await {
                        Ok(result) => {
                            if socket.send(Message::Binary(plain_rs::utils::base64::base64_decode(result["frame"].as_str().unwrap_or_default()))).await.is_err() { break; }
                            if result["status"] == "PENDING" {
                                let request_id = result["requestId"].as_str().unwrap().to_owned();
                                pending = Some(request_id.clone());
                                if let Some(c) = state.main_ws.connections.lock().unwrap().get_mut(&id) { c.pending = Some(request_id.clone()); }
                                notify_login_request(&state, &params.cid, &ip, &request_id, &result["request"]);
                            } else {
                                let mut result = result;
                                result["clientIp"] = json!(ip);
                                completed(&state, &result).await;
                            }
                            continue;
                        },
                        Err(error) => {
                            let reason = match error.to_string().as_str() { "too_many_login_attempts" => "too_many_login_attempts", "desktop_access_disabled" => "desktop_access_disabled", "login_expired" => "login_expired", _ if params.auth.as_deref() == Some("2") => "invalid_login_request", _ => "invalid_password" };
                            let _ = socket.send(close(1013,reason)).await;
                            break;
                        }
                    }
                }
                let Some(key) = crate::services::sessions::local_key(&state, &params.cid)
                    .or_else(|| crate::services::sessions::key(&state.db, &params.cid).ok().flatten())
                    else { let _ = socket.send(close(1013,"invalid_request")).await; break; };
                let plain = plain_rs::crypto::xchacha_decrypt_raw(&key,&frame);
                let Some(plain) = plain else { let _ = socket.send(close(1013,"invalid_request")).await; break; };
                let plain = match super::frames::admit(&state.main_ws.replay, &params.cid, &frame, &plain, registered, chrono::Utc::now().timestamp_millis()) {
                    Ok(payload) => payload,
                    Err(()) => { let _ = socket.send(close(1013,"invalid_request")).await; break; }
                };
                if !registered {
                    if channel == Channel::Screen {
                        if let Err(error) = crate::services::capabilities::require(&state.host, "SCREEN_MIRROR").await {
                            log::warn!("Screen mirror connection: {error}");
                            let _ = socket.send(close(1008, "unsupported_capability")).await;
                            break;
                        }
                    }
                    stream = Some(plain_rs::protocols::websocket::Stream::new(&frame));
                    registered = true;
                    permit.take();
                    if let Some(c) = state.main_ws.connections.lock().unwrap().get_mut(&id) { c.registered = true; }
                    if channel == Channel::ImageEditor {
                        let Some(bytes) = stream.as_mut().and_then(|stream| stream.encode("IMAGE_EDITOR_READY", b"", &key)) else { break; };
                        if socket.send(Message::Binary(bytes)).await.is_err() { break; }
                    }
                } else {
                    if channel != Channel::Screen {
                        let _ = socket.send(close(1008, "invalid_request")).await;
                        break;
                    }
                    let controls = controls(&plain);
                    if !controls.is_empty() {
                        if let Err(error) = crate::services::capabilities::require(&state.host, "SCREEN_MIRROR").await {
                            log::warn!("Screen mirror controls: {error}");
                            let _ = socket.send(close(1008, "unsupported_capability")).await;
                            break;
                        }
                        if let Err(error) = state.host.call("systemScreenMirrorControls", json!({"inputs":controls})).await {
                            log::warn!("Screen mirror controls: {error}");
                        }
                    }
                }
                continue;
            },
            event = receiver.recv(), if registered => {
                let event = match event { Ok(event) => event, Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { let _ = socket.send(close(1013,"slow_client")).await; break; }, Err(_) => break };
                let allowed = match channel {
                    Channel::Screen => is_screen_event(event.event_type),
                    Channel::ImageEditor => event.event_type == "IMAGE_EDITOR_UPDATE",
                    Channel::Auth => false,
                };
                if !allowed { continue; }
                if event.target_cid.as_ref().is_some_and(|cid| cid != &params.cid) { continue; }
                let Some(event) = event.into_public() else { continue; };
                if !enabled(&state) { break; }
                // Same key the inbound frames used: the shell holds no session.
                let Some(key) = crate::services::sessions::local_key(&state, &params.cid)
                    .or_else(|| crate::services::sessions::key(&state.db, &params.cid).ok().flatten())
                else { break; };
                let payload = if let Some(payload) = event.binary_payload {
                    payload
                } else {
                    event.payload.into_bytes()
                };
                let Some(bytes) = stream.as_mut().and_then(|stream| stream.encode(event.event_type, &payload, &key)) else { break; };
                Message::Binary(bytes)
            }
        };
        let closing = matches!(message, Message::Close(_));
        if !matches!(
            tokio::time::timeout(Duration::from_secs(2), socket.send(message)).await,
            Ok(Ok(()))
        ) || closing
        {
            break;
        }
    }
    if let Some(request_id) = pending {
        let _ = crate::http::auth::ws_login::execute(
            &state,
            crate::http::auth::ws_login::Request::Cancel { request_id },
        )
        .await;
    }
    state.main_ws.connections.lock().unwrap().remove(&id);
    if channel == Channel::Screen
        && registered
        && !state
            .main_ws
            .connections
            .lock()
            .unwrap()
            .values()
            .any(|c| c.channel == Channel::Screen && c.registered)
    {
        match crate::services::capabilities::snapshot(&state.host).await {
            Ok(capabilities) if capabilities.iter().any(|value| value == "SCREEN_MIRROR") => {
                if let Err(error) = state
                    .host
                    .call("systemScreenMirrorResetTouch", json!({}))
                    .await
                {
                    log::warn!("Screen mirror touch cleanup: {error}");
                }
            }
            Ok(_) => {}
            Err(error) => log::warn!("Screen mirror touch cleanup: {error}"),
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.close()).await;
}
fn notify_login_request(
    state: &ServerState,
    client_id: &str,
    client_ip: &str,
    request_id: &str,
    request: &Value,
) {
    let Ok(mut attempts) = state.login_attempts.lock() else {
        log::warn!("Web login state unavailable");
        return;
    };
    if !attempts.contains(request_id) {
        return;
    }
    let mut request = request.clone();
    request["password"] = json!("");
    attempts.notify_host(
        state,
        "systemWebLoginRequested",
        json!({"clientId":client_id,"clientIp":client_ip,"requestId":request_id,"request":request}),
    );
}

fn controls(bytes: &[u8]) -> Vec<Value> {
    if bytes.first() == Some(&0x54) {
        let Some(&count) = bytes.get(1) else {
            return vec![];
        };
        if count == 0 || bytes.len() < 4 + usize::from(count) * 8 {
            return vec![];
        }
        bytes[4..4 + usize::from(count)*8].chunks_exact(8).map(|p| json!({
            "action":match p[0] { 0=>"TOUCH_DOWN",1=>"TOUCH_MOVE",_=>"TOUCH_UP" },
            "pointerId":p[1],"x":f32::from(u16::from_le_bytes([p[2],p[3]]))/65535.0,"y":f32::from(u16::from_le_bytes([p[4],p[5]]))/65535.0
        })).collect()
    } else {
        let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
            return vec![];
        };
        if value["type"] != "screenMirrorControl" {
            return vec![];
        }
        let input = value["input"].clone();
        if !matches!(
            input["action"].as_str(),
            Some(
                "TAP"
                    | "LONG_PRESS"
                    | "SWIPE"
                    | "SCROLL"
                    | "BACK"
                    | "HOME"
                    | "RECENTS"
                    | "LOCK_SCREEN"
                    | "KEY"
                    | "TOUCH"
                    | "TOUCH_DOWN"
                    | "TOUCH_MOVE"
                    | "TOUCH_UP"
            )
        ) {
            return vec![];
        }
        vec![input]
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/http/websocket/runtime.rs"]
mod tests;
