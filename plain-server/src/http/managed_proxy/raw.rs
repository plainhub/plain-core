use super::{caller, current, decode_key};
use crate::server::ServerState;
use anyhow::{Result, anyhow, ensure};
use axum::{
    body::Body,
    extract::{FromRequestParts, Query, Request},
    http::{HeaderMap, HeaderName, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use plain_libs::protocols::{
    http_client,
    websocket::{CloseFrame, Message, WebSocketUpgrade},
};
use plain_rs::{
    features::{
        files::resource_link::{self, Scope},
        services::peers::peer_address,
    },
    infrastructure::db::{DPeer, SessionRow, chat_store::peers},
};
use std::{collections::HashMap, sync::Arc, time::Duration};

enum Target {
    Managed(Arc<super::PeerRoute>),
    Pairing {
        id: String,
        base: http_client::Url,
        peer: Option<DPeer>,
    },
}

impl Target {
    fn base(&self) -> &http_client::Url {
        match self {
            Self::Managed(route) => &route.base,
            Self::Pairing { base, .. } => base,
        }
    }

    fn check_current(&self, state: &ServerState) -> Result<()> {
        match self {
            Self::Managed(route) => route.check_current(state),
            Self::Pairing {
                id,
                peer: Some(peer),
                ..
            } => peer_address::current(&state.db, id, peer).map(|_| ()),
            Self::Pairing { id, peer: None, .. } => {
                ensure!(
                    state.nearby_devices.contains(id),
                    "Nearby device unavailable"
                );
                Ok(())
            }
        }
    }
}

pub(super) async fn call(
    state: ServerState,
    target: &str,
    path: &str,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    let Some((ticket, query)) = parts.uri.query().and_then(|query| {
        let (first, rest) = query.split_once('&').unwrap_or((query, ""));
        Some((first.strip_prefix("relay=")?, rest))
    }) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(raw_path) = upstream_path(&parts.uri) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let upstream_path = if query.is_empty() {
        format!("/{raw_path}")
    } else {
        format!("/{raw_path}?{query}")
    };
    let Ok(session) = authenticate(&state, ticket, target, &parts.method, &upstream_path) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let target = if parts.method == Method::POST && path == "init" {
        match pairing_target(&state, target) {
            Ok(target) => target,
            Err(status) => return status.into_response(),
        }
    } else {
        let Ok(route) = state.managed_proxy.route(&state, target).await else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        Target::Managed(route)
    };
    let client = match &target {
        Target::Managed(route) => route.client.clone(),
        Target::Pairing { .. } => state.lan.client.clone(),
    };
    let Ok(url) = target_url(target.base().clone(), raw_path, query) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    relay(
        client,
        url,
        Request::from_parts(parts, body),
        Access::Remote(state, session, target),
    )
    .await
}

fn pairing_target(state: &ServerState, target: &str) -> Result<Target, StatusCode> {
    if state.native_client_id().is_ok_and(|id| id == target) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let peer = peers::get(&state.db, target).map_err(|_| StatusCode::BAD_GATEWAY)?;
    let base = if let Some(peer) = &peer {
        http_client::Url::parse(&peer_address::address(peer).base_url).map_err(anyhow::Error::from)
    } else {
        plain_rs::features::services::discovery::pairing_attempts::nearby_management_address(
            state, target,
        )
    }
    .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Target::Pairing {
        id: target.to_owned(),
        base,
        peer,
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthQuery {
    cid: String,
    auth: Option<String>,
}

pub(super) async fn auth(state: ServerState, target: String, request: Request) -> Response {
    let Ok(Query(parameters)) = Query::<AuthQuery>::try_from_uri(request.uri()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !matches!(parameters.auth.as_deref(), Some("1" | "2")) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(session) = caller(&state, &parameters.cid) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(key) = decode_key(&session.token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let binding = request.uri().path_and_query().unwrap().as_str().to_owned();
    let query = request.uri().query().unwrap_or_default().to_owned();
    let (mut parts, _) = request.into_parts();
    let Ok(upgrade) = WebSocketUpgrade::from_request_parts(&mut parts, &()).await else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    upgrade
        .max_message_size(65536)
        .max_frame_size(65536)
        .on_upgrade(move |mut socket| async move {
            let result = async {
                let (_, ack) = crate::http::websocket::graphql::acknowledge(
                    &mut socket,
                    &state,
                    &session.client_id,
                    &key,
                    &binding,
                )
                .await
                .map_err(anyhow::Error::msg)?;
                current(&state, &session)?;
                let target = pairing_target(&state, &target)
                    .map_err(|_| anyhow!("Pairing device unavailable"))?;
                let mut url = target_url(target.base().clone(), "auth", &query)?;
                url.set_scheme("wss")
                    .map_err(|_| anyhow!("Invalid pairing address"))?;
                let mut upstream = tokio::time::timeout(
                    Duration::from_secs(5),
                    plain_rs::features::services::peers::status_socket::connect(url.as_str()),
                )
                .await??;
                let access = Access::Remote(state, session, target);
                ensure!(access.valid(), "Pairing access revoked");
                tokio::time::timeout(
                    Duration::from_secs(2),
                    socket.send(Message::Binary(ack.into())),
                )
                .await??;
                let mut recheck = tokio::time::interval(Duration::from_secs(1));
                recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    let (message, upstream_direction) = tokio::select! {
                        _ = recheck.tick() => {
                            ensure!(access.valid(), "Pairing access revoked");
                            continue;
                        }
                        next = socket.next() => (next, true),
                        next = upstream.next() => (next, false),
                    };
                    match message.transpose()? {
                        Some(frame @ (Message::Text(_) | Message::Binary(_))) => {
                            if upstream_direction {
                                tokio::time::timeout(Duration::from_secs(2), upstream.send(frame))
                                    .await??;
                            } else {
                                tokio::time::timeout(Duration::from_secs(2), socket.send(frame))
                                    .await??;
                            }
                        }
                        Some(Message::Close(frame)) => {
                            if upstream_direction {
                                let _ = upstream.close(frame).await;
                            } else {
                                let _ = socket.close(frame).await;
                            }
                            break;
                        }
                        Some(_) => {}
                        None => break,
                    }
                }
                Ok::<_, anyhow::Error>(())
            }
            .await;
            if result.is_err() {
                let _ = socket
                    .close(Some(CloseFrame {
                        code: 1008u16.into(),
                        reason: "proxy_auth_failed".into(),
                    }))
                    .await;
            }
        })
}

fn native_peer(
    state: &ServerState,
    caller: &str,
    target: &str,
) -> Result<http_client::Url, StatusCode> {
    if !state.native_client_id().is_ok_and(|id| id == caller) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let peer = match peers::get(&state.db, target) {
        Ok(Some(peer)) if peer.id != caller && peer.is_paired() && !peer.token.is_empty() => peer,
        Ok(_) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::BAD_GATEWAY),
    };
    http_client::Url::parse(&peer_address::address(&peer).base_url)
        .map_err(|_| StatusCode::BAD_GATEWAY)
}

// Browsers cannot set headers on a WebSocket, so the desktop client id travels in `cid`.
pub(super) async fn native_socket(state: ServerState, target: &str, request: Request) -> Response {
    let Ok(Query(parameters)) = Query::<HashMap<String, String>>::try_from_uri(request.uri())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let caller = parameters.get("cid").map_or("", String::as_str);
    match native_peer(&state, caller, target) {
        Ok(base) => native_relay(state, base, request, true).await,
        Err(status) => status.into_response(),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeTarget {
    base: String,
}

pub(super) async fn native(state: ServerState, ticket: &str, request: Request) -> Response {
    if request.headers().contains_key(header::UPGRADE) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok((Scope::Session(owner), payload)) = resource_link::parse(ticket) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !state.native_client_id().is_ok_and(|id| id == owner) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(plain) = resource_link::decrypt(
        &Scope::Session(owner),
        "native-proxy",
        &state.url_key,
        payload,
    ) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(target) = serde_json::from_slice::<NativeTarget>(&plain) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(base) = http_client::Url::parse(&target.base) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !matches!(base.scheme(), "http" | "https")
        || base.origin().ascii_serialization() != target.base
    {
        return StatusCode::BAD_REQUEST.into_response();
    }
    native_relay(state, base, request, false).await
}

async fn native_relay(
    state: ServerState,
    base: http_client::Url,
    request: Request,
    rediscover: bool,
) -> Response {
    let Some(raw_path) = upstream_path(request.uri()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(url) = target_url(base, raw_path, request.uri().query().unwrap_or_default()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let client = state
        .managed_proxy
        .native_client
        .get_or_init(|| {
            http_client::builder()
                .no_proxy()
                .redirect(http_client::redirect::Policy::none())
                .danger_accept_invalid_certs(true)
                .tcp_keepalive(Duration::from_secs(60))
                .pool_max_idle_per_host(20)
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("native proxy HTTP client")
        })
        .clone();
    relay(client, url, request, Access::Native(state, rediscover)).await
}

enum Access {
    Remote(ServerState, SessionRow, Target),
    Native(ServerState, bool),
}

impl Access {
    fn valid(&self) -> bool {
        match self {
            Self::Remote(state, session, target) => {
                current(state, session).is_ok() && target.check_current(state).is_ok()
            }
            Self::Native(_, _) => true,
        }
    }
}

impl Drop for Access {
    fn drop(&mut self) {
        if let Self::Native(state, true) = self {
            state.mdns.browse_resident();
        }
    }
}

fn upstream_path(uri: &axum::http::Uri) -> Option<&str> {
    uri.path()
        .strip_prefix("/proxy/")?
        .split_once('/')
        .map(|(_, path)| path)
}

fn target_url(mut base: http_client::Url, path: &str, query: &str) -> Result<http_client::Url> {
    base.set_path(&format!("/{path}"));
    ensure!(
        base.path().strip_prefix('/') == Some(path),
        "Invalid upstream path"
    );
    base.set_query((!query.is_empty()).then_some(query));
    Ok(base)
}

async fn relay(
    client: http_client::Client,
    url: http_client::Url,
    request: Request,
    access: Access,
) -> Response {
    if !access.valid() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let (mut parts, body) = request.into_parts();
    let websocket = parts
        .headers
        .get(header::UPGRADE)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));
    let mut outgoing = client.request(parts.method.clone(), url);
    for (name, value) in &parts.headers {
        if !blocked_header(&parts.headers, name)
            && !matches!(
                *name,
                header::AUTHORIZATION
                    | header::COOKIE
                    | header::HOST
                    | header::ORIGIN
                    | header::REFERER
            )
            && !name.as_str().starts_with("x-plain-")
        {
            outgoing = outgoing.header(name, value);
        }
    }
    let upgrade = if websocket {
        let Ok(upgrade) = WebSocketUpgrade::from_request_parts(&mut parts, &()).await else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        outgoing = outgoing
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket");
        Some(upgrade)
    } else {
        outgoing = outgoing.body(http_client::Body::wrap_stream(body.into_data_stream()));
        None
    };
    let upstream = outgoing.send().await;
    if !access.valid() {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(upgrade) = upgrade {
        if let Ok(upstream) = upstream {
            if upstream.status() == StatusCode::SWITCHING_PROTOCOLS {
                let headers = upstream.headers().clone();
                if let Ok(upstream) = upstream.upgrade().await {
                    return upgrade.relay(&headers, upstream, async move {
                        if matches!(&access, Access::Native(_, _)) {
                            std::future::pending::<()>().await;
                        }
                        let mut recheck = tokio::time::interval(Duration::from_secs(1));
                        recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                        loop {
                            recheck.tick().await;
                            if !access.valid() {
                                break;
                            }
                        }
                    });
                }
            }
        }
        return if matches!(access, Access::Native(_, _)) {
            upgrade.on_upgrade(move |mut socket| async move {
                let _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: 1011u16.into(),
                        reason: "upstream unavailable".into(),
                    })))
                    .await;
                drop(access);
            })
        } else {
            StatusCode::BAD_GATEWAY.into_response()
        };
    }
    let Ok(upstream) = upstream else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status = upstream.status();
    if status == StatusCode::SWITCHING_PROTOCOLS
        || (status.is_redirection() && status != StatusCode::NOT_MODIFIED)
    {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let cache = matches!(access, Access::Native(_, _))
        .then(|| upstream.headers().get(header::CACHE_CONTROL))
        .flatten()
        .cloned()
        .unwrap_or_else(|| axum::http::HeaderValue::from_static("no-store"));
    let mut response = Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, cache)
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'",
        );
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_DISPOSITION,
        header::CONTENT_ENCODING,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
        header::ETAG,
        header::LAST_MODIFIED,
        header::RETRY_AFTER,
    ] {
        if let Some(value) = upstream
            .headers()
            .get(&name)
            .filter(|_| !blocked_header(upstream.headers(), &name))
        {
            response = response.header(name, value);
        }
    }
    response
        .body(Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

fn blocked_header(headers: &HeaderMap, name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "upgrade"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "transfer-encoding"
    ) || headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| name.as_str().eq_ignore_ascii_case(value.trim()))
}

fn authenticate(
    state: &ServerState,
    ticket: &str,
    target: &str,
    method: &Method,
    path: &str,
) -> Result<SessionRow> {
    let (Scope::Session(id), payload) = resource_link::parse(ticket)? else {
        return Err(anyhow!("Invalid relay scope"));
    };
    let session = caller(state, &id)?;
    let key = decode_key(&session.url_token)?;
    let purpose = format!("relay:{target}:{method}:{path}");
    let plain = resource_link::decrypt(&Scope::Session(id), &purpose, &key, payload)?;
    let issued_at: i64 = serde_json::from_slice(&plain)?;
    ensure!(
        (0..=60_000).contains(
            &chrono::Utc::now()
                .timestamp_millis()
                .saturating_sub(issued_at)
        ),
        "Relay ticket expired"
    );
    Ok(session)
}
