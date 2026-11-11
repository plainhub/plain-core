use super::{caller, current, decode_key};
use crate::server::ServerState;
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::axum::{
    body::Body,
    extract::{FromRequestParts, Query, Request},
    http::{HeaderMap, HeaderName, StatusCode, header},
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
    infrastructure::db::{DPeer, chat_store::peers},
};
use std::{collections::HashMap, time::Duration};

struct PairingTarget {
    id: String,
    base: http_client::Url,
    peer: Option<DPeer>,
}

impl PairingTarget {
    fn check_current(&self, state: &ServerState) -> Result<()> {
        if let Some(peer) = &self.peer {
            peer_address::current(&state.db, &self.id, peer)?;
        } else {
            ensure!(
                state.nearby_devices.contains(&self.id),
                "Nearby device unavailable"
            );
        }
        Ok(())
    }
}

fn pairing_target(state: &ServerState, target: &str) -> Result<PairingTarget, StatusCode> {
    if state.native_client_id().is_ok_and(|id| id == target) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let peer = peers::get(&state.db, target).map_err(|_| StatusCode::BAD_GATEWAY)?;
    let base = if let Some(peer) = &peer {
        http_client::Url::parse(&peer_address::address(peer).base_url).map_err(plain_libs::anyhow::Error::from)
    } else {
        plain_rs::features::services::discovery::pairing_attempts::nearby_management_address(
            state, target,
        )
    }
    .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(PairingTarget {
        id: target.to_owned(),
        base,
        peer,
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthQuery {
    cid: String,
}

pub(super) async fn auth(state: ServerState, target: String, request: Request) -> Response {
    let Ok(Query(parameters)) = Query::<AuthQuery>::try_from_uri(request.uri()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
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
                .map_err(plain_libs::anyhow::Error::msg)?;
                current(&state, &session)?;
                let target = pairing_target(&state, &target)
                    .map_err(|_| anyhow!("Pairing device unavailable"))?;
                let mut url = target_url(target.base.clone(), "auth", &query)?;
                url.set_scheme("wss")
                    .map_err(|_| anyhow!("Invalid pairing address"))?;
                let mut upstream = tokio::time::timeout(
                    Duration::from_secs(5),
                    plain_rs::features::services::peers::status_socket::connect(url.as_str()),
                )
                .await??;
                current(&state, &session)?;
                target.check_current(&state)?;
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
                            current(&state, &session)?;
                            target.check_current(&state)?;
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
                Ok::<_, plain_libs::anyhow::Error>(())
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
    let Ok(session) = plain_rs::features::services::sessions::native(&state) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(key) = decode_key(&session.url_token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(plain) = resource_link::decrypt(&Scope::Session(owner), "native-proxy", &key, payload)
    else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(target) = plain_libs::serde_json::from_slice::<NativeTarget>(&plain) else {
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
    relay(client, url, request, NativeAccess { state, rediscover }).await
}

struct NativeAccess {
    state: ServerState,
    rediscover: bool,
}

impl Drop for NativeAccess {
    fn drop(&mut self) {
        if self.rediscover {
            self.state.mdns.browse_resident();
        }
    }
}

fn upstream_path(uri: &plain_libs::axum::http::Uri) -> Option<&str> {
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
    access: NativeAccess,
) -> Response {
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
    if let Some(upgrade) = upgrade {
        if let Ok(upstream) = upstream {
            if upstream.status() == StatusCode::SWITCHING_PROTOCOLS {
                let headers = upstream.headers().clone();
                if let Ok(upstream) = upstream.upgrade().await {
                    return upgrade.relay(&headers, upstream, async move {
                        let _access = access;
                        std::future::pending::<()>().await;
                    });
                }
            }
        }
        return upgrade.on_upgrade(move |mut socket| async move {
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: 1011u16.into(),
                    reason: "upstream unavailable".into(),
                })))
                .await;
            drop(access);
        });
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
    let cache = upstream
        .headers()
        .get(header::CACHE_CONTROL)
        .cloned()
        .unwrap_or_else(|| plain_libs::axum::http::HeaderValue::from_static("no-store"));
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
