//! `GET /proxyfs` — proxies a peer HTTP URL for the main web UI.
//!
//! The `id` query parameter is a urlToken-encrypted absolute URL, matching
//! plain-app's `UrlHelper.decrypt` (base64 of the XChaCha20-Poly1305 blob).
//! Only the client that holds the urlToken can mint one, so the decrypted value
//! is a peer URL chosen by the UI, not attacker input.

use crate::server::ServerState;
use axum::{
    body::Body,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, HeaderName, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use std::net::SocketAddr;

fn remote_access_allowed(state: &ServerState) -> bool {
    state.prefs.get_or("remote_access", true)
}

/// The decrypted value is handed straight to an outbound client, so accept only
/// the two schemes a peer file URL can legitimately use.
fn target_url(plaintext: &str) -> anyhow::Result<&str> {
    let trimmed = plaintext.trim();
    anyhow::ensure!(
        trimmed.starts_with("http://") || trimmed.starts_with("https://"),
        "Invalid peer URL"
    );
    let authority = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    anyhow::ensure!(!authority.is_empty(), "Invalid peer URL");
    Ok(trimmed)
}

fn decode_target(state: &ServerState, id: &str) -> anyhow::Result<String> {
    let url = String::from_utf8(plain_rs::features::services::files::links::decrypt(
        state, id, "proxyfs",
    )?)?;
    Ok(target_url(&url)?.to_owned())
}

/// Hop-by-hop headers must not be copied through a proxy, and the upstream
/// `content-encoding`/`content-length` do not describe the body we re-stream.
const SKIPPED: &[&str] = &[
    "connection",
    "transfer-encoding",
    "upgrade",
    "content-encoding",
    "content-length",
];

pub(crate) async fn call(
    State(state): State<ServerState>,
    _remote: ConnectInfo<SocketAddr>,
    Query(params): Query<crate::http::fs::Parameters>,
    headers: HeaderMap,
) -> Response {
    if !remote_access_allowed(&state) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !params.is_valid() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let url = match decode_target(&state, &params.id) {
        Ok(url) => url,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("File is expired or does not exist. {error}"),
            )
                .into_response();
        }
    };
    let url = match resource_target(&url, &params) {
        Ok(url) => url,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    proxy(&url, &headers).await
}
fn resource_target(url: &str, parameters: &crate::http::fs::Parameters) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(url)?;
    anyhow::ensure!(url.path() == "/fs", "File resource URL required");
    let id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| anyhow::anyhow!("File resource ID required"))?;
    url.set_query(None);
    let mut query = url.query_pairs_mut();
    query
        .append_pair("id", &id)
        .append_pair("action", parameters.action.as_str());
    if let Some(size) = parameters.size {
        query.append_pair(
            "size",
            match size {
                plain_rs::features::services::files::content::Size::Small => "sm",
                plain_rs::features::services::files::content::Size::Large => "lg",
            },
        );
    }
    if let Some(version) = &parameters.version {
        query.append_pair("v", version);
    }
    drop(query);
    Ok(url.to_string())
}

pub(crate) async fn proxy(url: &str, headers: &HeaderMap) -> Response {
    let client = match reqwest::Client::builder().build() {
        Ok(client) => client,
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
        }
    };
    let mut request = client.get(url);
    for name in [
        header::USER_AGENT,
        header::RANGE,
        header::IF_RANGE,
        header::IF_NONE_MATCH,
    ] {
        if let Some(value) = headers.get(&name) {
            request = request.header(name, value);
        }
    }
    let upstream = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
        }
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);
    for (name, value) in upstream.headers() {
        let name = name.as_str();
        if SKIPPED.contains(&name) {
            continue;
        }
        if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
            response = response.header(name, value.clone());
        }
    }
    let stream = upstream
        .bytes_stream()
        .map(|chunk| chunk.map_err(std::io::Error::other));
    match response.body(Body::from_stream(stream)) {
        Ok(response) => response,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/http/proxy.rs"]
mod tests;

#[cfg(feature = "core")]
pub mod local;
