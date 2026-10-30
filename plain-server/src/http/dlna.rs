use plain_rs::prefs::pref_keys::{DLNA_ALLOWED_SENDERS, DLNA_DENIED_SENDERS};
use crate::assembly::server::ServerState;
use crate::services::casting::receiver::control::emit;
#[cfg(test)]
use crate::services::casting::receiver::control::snapshot;
use axum::{
    extract::{ConnectInfo, Path, Request as HttpRequest, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
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
async fn read_body(body: axum::body::Body, limit: usize) -> Result<Vec<u8>, BodyFailure> {
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
    plain_rs::prefs::dlna::enabled(&state.prefs)
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

use crate::services::casting::address::device_name;

/// The address the SSDP LOCATION points at and the device description
/// advertises. Must agree with the one the advertiser used, or a control
/// point fetches the description from an address the renderer never joined.
fn local_ip() -> String {
    let ips = plain_rs::protocols::mdns::host_responder::local_ipv4_strs();
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

    let allowed = plain_rs::prefs::dlna::senders(&state.prefs, DLNA_ALLOWED_SENDERS);
    let denied = plain_rs::prefs::dlna::senders(&state.prefs, DLNA_DENIED_SENDERS);
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

    let response = crate::services::casting::receiver::http_router::route(
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
    out.body(axum::body::Body::from(response.body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../tests/unit/http/dlna.rs"]
mod tests;

pub(crate) async fn media(
    State(state): State<ServerState>,
    Path(raw): Path<String>,
    request: HttpRequest,
) -> Response {
    let id = raw.split('.').next().unwrap_or_default();
    let Some((path, mime)) = crate::services::casting::media_alias::lookup(id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if path.starts_with("http://") || path.starts_with("https://") {
        return crate::http::file_response::secure(
            crate::http::proxy::proxy(&path, request.headers()).await,
            None,
            false,
        );
    }
    if path.starts_with("content://") {
        #[cfg(feature = "http_transport")]
        {
            return match state.bridge.resource(&path, state.stop.clone()).await {
                Ok(body) => crate::http::file_response::secure(body.into_response(), None, false),
                Err(status) => status.into_response(),
            };
        }
        #[cfg(not(feature = "http_transport"))]
        {
            return StatusCode::NOT_FOUND.into_response();
        }
    }
    let path = match crate::services::files::references::resolve(state.db.clone(), state.directory.clone(), &path) {
        Ok(path) => path,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let mut response = crate::http::file_response::stream_file(
        std::path::Path::new(&path),
        &mime,
        request.headers(),
    )
    .await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    if !mime.starts_with("image/")
        && (response.status() == StatusCode::OK || response.status() == StatusCode::PARTIAL_CONTENT)
    {
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
        response
            .headers_mut()
            .insert("realTimeInfo.dlna.org", "DLNA.ORG_TLAG=*".parse().unwrap());
        response
            .headers_mut()
            .insert("Server", "DLNADOC/1.50 UPnP/1.0 Plain/1.0".parse().unwrap());
        response
            .headers_mut()
            .insert("transfermode.dlna.org", "Streaming".parse().unwrap());
        response
            .headers_mut()
            .insert("contentfeatures.dlna.org", "".parse().unwrap());
    }
    crate::http::file_response::secure(response, None, false)
}

pub(crate) async fn callback(State(state): State<ServerState>, request: HttpRequest) -> Response {
    if request.method().as_str() != "NOTIFY" {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let sid = request
        .headers()
        .get("SID")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let sequence = request
        .headers()
        .get("SEQ")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok());
    let Ok(body) = axum::body::to_bytes(request.into_body(), 1024 * 1024).await else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let xml = String::from_utf8_lossy(&body);
    let fields = crate::services::casting::cast_runtime::attributes(&xml);
    let _guard = state.cast.operations.lock().await;
    if sid
        .as_ref()
        .is_some_and(|sid| sid != &state.cast.snapshot().sid)
    {
        return StatusCode::OK.into_response();
    }
    if let Some(sequence) = sequence {
        let mut previous = state.cast.callback_sequence.lock().unwrap();
        if previous.is_some_and(|previous| {
            sequence.wrapping_sub(previous) == 0 || sequence.wrapping_sub(previous) >= (1 << 31)
        }) {
            return StatusCode::OK.into_response();
        }
        *previous = Some(sequence);
    }
    let outcome = crate::services::casting::cast_runtime::apply(&state, &fields).await;
    if let Err(error) = outcome {
        log::warn!("Cast callback failed: {error}");
    }
    StatusCode::OK.into_response()
}
