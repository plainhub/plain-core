use crate::server::ServerState;
use plain_libs::axum::{
    extract::{ConnectInfo, Path, Request as HttpRequest, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use plain_libs::protocols::http_client;
use plain_rs::features::prefs::pref_keys::{DLNA_ALLOWED_SENDERS, DLNA_DENIED_SENDERS};
use plain_rs::features::services::casting::receiver::control::emit;
#[cfg(test)]
use plain_rs::features::services::casting::receiver::control::snapshot;
use std::{collections::HashMap, net::SocketAddr};

/// Control points send a small description document; a larger cap than they
/// need would only widen the surface for a stray LAN client.
const MAX_BODY: usize = 2 * 1024 * 1024;

/// An over-limit body and a body that failed mid-read are different faults, and
/// neither may reach the protocol as the empty request it would otherwise look
/// like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyFailure {
    TooLarge,
    Unreadable,
}

impl From<BodyFailure> for StatusCode {
    fn from(failure: BodyFailure) -> Self {
        match failure {
            BodyFailure::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            BodyFailure::Unreadable => StatusCode::BAD_REQUEST,
        }
    }
}

/// Reads the body under [MAX_BODY] and reports which limit was hit, which the
/// buffered extractor cannot: it folds both into one error.
async fn read_body(body: plain_libs::axum::body::Body, limit: usize) -> Result<Vec<u8>, BodyFailure> {
    let mut stream = body.into_data_stream();
    let mut buffer = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| BodyFailure::Unreadable)?;
        if buffer.len() + chunk.len() > limit {
            return Err(BodyFailure::TooLarge);
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer)
}

fn access_allowed(state: &ServerState) -> bool {
    plain_rs::features::prefs::dlna::enabled(&state.prefs)
}

fn headers_to_map(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect()
}

use plain_rs::features::services::casting::address::device_name;

/// The address the SSDP LOCATION points at and the device description
/// advertises. Must agree with the one the advertiser used, or a control
/// point fetches the description from an address the renderer never joined.
fn local_ip() -> String {
    let ips = plain_rs::infrastructure::protocols::mdns::host_responder::local_ipv4_strs();
    ips.iter()
        .find(|ip| !ip.starts_with("127."))
        .or_else(|| ips.first())
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".into())
}

/// Public DLNA receiver endpoint. One handler for every UPnP path — the
/// router itself decides by method and path suffix, mirroring plain-app's
/// `DlnaHttpRouter`.
pub(crate) async fn receiver(
    State(state): State<ServerState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: HttpRequest,
) -> Response {
    if !access_allowed(&state) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (parts, body) = request.into_parts();
    // A body that never arrived intact must not be answered as an empty SOAP
    // request, and it must not reach the router or the command channel.
    let bytes = match read_body(body, MAX_BODY).await {
        Ok(bytes) => bytes,
        Err(failure) => return StatusCode::from(failure).into_response(),
    };
    let body = String::from_utf8_lossy(&bytes).to_string();
    let headers = headers_to_map(&parts.headers);

    let allowed = plain_rs::features::prefs::dlna::senders(&state.prefs, DLNA_ALLOWED_SENDERS);
    let denied = plain_rs::features::prefs::dlna::senders(&state.prefs, DLNA_DENIED_SENDERS);
    // The allow/deny rules and the "remember this sender" prefs both key on
    // the peer address, so it comes from the socket — a control point that
    // forged `c-ip` would otherwise be able to impersonate a trusted sender.
    let sender_ip = peer.ip().to_string();

    let name = match device_name(&state).await {
        Ok(name) => name,
        Err(error) => {
            log::error!("DLNA device name failed: {error}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response = plain_rs::features::services::casting::receiver::http_router::route(
        &state.dlna.state,
        parts.method.as_str(),
        parts.uri.path(),
        &headers,
        &body,
        state.dlna.device_uuid(),
        &name,
        &local_ip(),
        &sender_ip,
        // The channel exists from construction: these routes answer as soon
        // as the toggle is on, which can be before the app starts the engine.
        &state.dlna.command_sender().expect("command channel"),
        &allowed,
        &denied,
    )
    .await;

    let renderer = state.dlna.snapshot().await;
    emit(&state, &renderer);

    let mut out = Response::builder()
        .status(StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
    for (name, value) in &response.headers {
        out = out.header(name.as_str(), value.as_str());
    }
    if let Some(content_type) = &response.content_type {
        out = out.header("content-type", content_type.as_str());
    }
    out.body(plain_libs::axum::body::Body::from(response.body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/plain-server/unit/http/dlna.rs"]
mod tests;

pub(crate) async fn media(
    State(state): State<ServerState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path(name): Path<String>,
    request: HttpRequest,
) -> Response {
    let Some(resource) = state.cast.media.lookup(&name, peer.ip()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (parts, _) = request.into_parts();
    let cancelled = resource.clone().cancelled();
    let mut response = tokio::select! {
        biased;
        _ = cancelled => return StatusCode::NOT_FOUND.into_response(),
        response = media_response(&state, &resource, &parts.method, &parts.headers) => response,
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    if response.status().is_success() {
        response
            .headers_mut()
            .insert("realTimeInfo.dlna.org", "DLNA.ORG_TLAG=*".parse().unwrap());
        response
            .headers_mut()
            .insert("Server", "DLNADOC/1.50 UPnP/1.0 Plain/1.0".parse().unwrap());
        response
            .headers_mut()
            .insert("transfermode.dlna.org", "Streaming".parse().unwrap());
    }
    let response = crate::http::file_response::secure(response, None, false);
    let head = parts.method == plain_libs::axum::http::Method::HEAD;
    let (parts, body) = response.into_parts();
    let body = if head {
        plain_libs::axum::body::Body::empty()
    } else {
        plain_libs::axum::body::Body::from_stream(body.into_data_stream().take_until(resource.cancelled()))
    };
    Response::from_parts(parts, body)
}

async fn media_response(
    state: &ServerState,
    resource: &plain_rs::features::services::casting::media_alias::Resource,
    method: &plain_libs::axum::http::Method,
    headers: &HeaderMap,
) -> Response {
    let path = &resource.path;
    if path.starts_with("http://") || path.starts_with("https://") {
        return remote_media(path, method, headers).await;
    }
    if path.starts_with("content://") {
        let Ok(input) = state.resources.read_file(path, state.stop.clone()).await else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        let Ok(size) = u64::try_from(input.length) else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        return crate::http::file_response::stream_open_file(
            tokio::fs::File::from_std(input.file),
            input.offset as u64,
            size,
            &resource.mime,
            headers,
        )
        .await;
    }
    let path = match plain_rs::features::services::files::references::resolve(
        state.db.clone(),
        state.directory.clone(),
        path,
    ) {
        Ok(path) => path,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    crate::http::file_response::stream_file(std::path::Path::new(&path), &resource.mime, headers)
        .await
}

pub(crate) async fn callback(
    State(state): State<ServerState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: HttpRequest,
) -> Response {
    if request.method().as_str() != "NOTIFY" {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let Some(sid) = request
        .headers()
        .get("SID")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
    else {
        return StatusCode::PRECONDITION_FAILED.into_response();
    };
    let Some(sequence) = request
        .headers()
        .get("SEQ")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok())
    else {
        return StatusCode::PRECONDITION_FAILED.into_response();
    };
    if !state.cast.media.accepts(peer.ip()) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    let Ok(body) = plain_libs::axum::body::to_bytes(request.into_body(), 1024 * 1024).await else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let xml = String::from_utf8_lossy(&body);
    let fields = plain_rs::features::services::casting::cast_runtime::attributes(&xml);
    let _guard = state.cast.operations.lock().await;
    if sid != state.cast.snapshot().sid || !state.cast.media.accepts(peer.ip()) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    {
        let mut previous = state.cast.callback_sequence.lock().unwrap();
        if previous.is_some_and(|previous| {
            sequence.wrapping_sub(previous) == 0 || sequence.wrapping_sub(previous) >= (1 << 31)
        }) {
            return StatusCode::OK.into_response();
        }
        *previous = Some(sequence);
    }
    let outcome = plain_rs::features::services::casting::cast_runtime::apply(&state, &fields).await;
    if let Err(error) = outcome {
        log::warn!("Cast callback failed: {error}");
    }
    StatusCode::OK.into_response()
}

fn remote_address_allowed(ip: std::net::IpAddr) -> bool {
    let ip = ip.to_canonical();
    let allowed = match ip {
        std::net::IpAddr::V4(ip) => {
            !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_multicast()
                && !ip.is_broadcast()
                && ip.octets()[0] != 0
                && ip.octets()[0] < 240
                && !(ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
                && ip.octets() != [168, 63, 129, 16]
        }
        std::net::IpAddr::V6(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_unicast_link_local()
                && !ip.is_multicast()
                && ip.segments() != [0xfd00, 0xec2, 0, 0, 0, 0, 0, 0x254]
        }
    };
    allowed
        && !plain_rs::infrastructure::protocols::mdns::host_responder::is_local_ipv6(
            &ip.to_string(),
        )
        && !plain_rs::infrastructure::protocols::mdns::host_responder::local_ipv4_strs()
            .iter()
            .filter_map(|local| local.parse::<std::net::IpAddr>().ok())
            .any(|local| local == ip)
}

fn remote_target_allowed(
    address: SocketAddr,
    host: &str,
    initial_host: &str,
    initial_addresses: Option<&[SocketAddr]>,
) -> bool {
    let ip = address.ip().to_canonical();
    let lan = match ip {
        std::net::IpAddr::V4(ip) => ip.is_private(),
        std::net::IpAddr::V6(ip) => ip.is_unique_local(),
    };
    remote_address_allowed(ip)
        && (!lan
            || initial_addresses.is_none_or(|addresses| {
                host.trim_matches(['[', ']']) == initial_host.trim_matches(['[', ']'])
                    && addresses.contains(&address)
            }))
}

async fn remote_media(url: &str, method: &plain_libs::axum::http::Method, headers: &HeaderMap) -> Response {
    let Ok(mut url) = http_client::Url::parse(url) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let initial_host = url.host_str().unwrap_or_default().to_owned();
    let mut initial_addresses = Vec::new();
    for attempt in 0..=5 {
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        let Some(host) = url.host_str() else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        let host = host.trim_matches(['[', ']']);
        let Some(port) = url.port_or_known_default() else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        let Ok(addresses) = tokio::net::lookup_host((host, port)).await else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        let addresses: Vec<_> = addresses.collect();
        if addresses.is_empty()
            || addresses.iter().any(|address| {
                !remote_target_allowed(
                    *address,
                    host,
                    &initial_host,
                    (attempt != 0).then_some(initial_addresses.as_slice()),
                )
            })
        {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        if attempt == 0 {
            initial_addresses = addresses.clone();
        }
        let upstream = match remote_fetch(url.clone(), method, headers, addresses[0]).await {
            Ok(upstream) => upstream,
            Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
        };
        if upstream.status().is_redirection() {
            let Some(next) = upstream
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|location| url.join(location).ok())
            else {
                return StatusCode::BAD_GATEWAY.into_response();
            };
            url = next;
            continue;
        }
        return remote_response(upstream, method);
    }
    StatusCode::BAD_GATEWAY.into_response()
}

async fn remote_fetch(
    url: http_client::Url,
    method: &plain_libs::axum::http::Method,
    headers: &HeaderMap,
    address: SocketAddr,
) -> Result<http_client::Response, http_client::Error> {
    let client = match http_client::builder()
        .no_proxy()
        .redirect(http_client::redirect::Policy::none())
        .resolve(url.host_str().unwrap().trim_matches(['[', ']']), address)
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(error) => return Err(error),
    };
    let mut request = client.request(method.clone(), url);
    for name in [header::RANGE, header::IF_RANGE] {
        if let Some(value) = headers.get(&name) {
            request = request.header(name, value);
        }
    }
    request.send().await
}

fn remote_response(upstream: http_client::Response, method: &plain_libs::axum::http::Method) -> Response {
    let status = upstream.status();
    if !matches!(status.as_u16(), 200 | 206 | 416) {
        return StatusCode::BAD_GATEWAY.into_response();
    }
    let mut response = Response::builder().status(status);
    for name in [
        header::CONTENT_TYPE,
        header::CONTENT_LENGTH,
        header::CONTENT_RANGE,
        header::ACCEPT_RANGES,
    ] {
        if let Some(value) = upstream.headers().get(&name) {
            response = response.header(name, value.clone());
        }
    }
    let body = if method == plain_libs::axum::http::Method::HEAD {
        plain_libs::axum::body::Body::empty()
    } else {
        plain_libs::axum::body::Body::from_stream(
            upstream
                .bytes_stream()
                .map(|chunk| chunk.map_err(std::io::Error::other)),
        )
    };
    response
        .body(body)
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}
