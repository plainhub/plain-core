use super::{ROUTE_LIFETIME, current, decode_pin, relay};
use crate::server::ServerState;
use anyhow::{Result, anyhow, ensure};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use plain_libs::protocols::http_client;
use plain_rs::{
    features::services::peers::{peer_address, peer_lan},
    infrastructure::db::{DPeer, chat_store::peers},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) type RouteSlot = Arc<tokio::sync::Mutex<Option<Arc<Route>>>>;

pub(super) struct Route {
    peer: DPeer,
    base: http_client::Url,
    pin: Option<[u8; 32]>,
    client: http_client::Client,
    expires: Instant,
}

pub(super) async fn call(
    state: &ServerState,
    target: &str,
    mut parameters: crate::http::fs::Parameters,
    reference: &[u8],
    session: Option<&plain_rs::infrastructure::db::SessionRow>,
    headers: &axum::http::HeaderMap,
) -> Response {
    let Some(id) = std::str::from_utf8(reference)
        .ok()
        .and_then(|value| value.strip_prefix("fsid:"))
        .filter(|id| !id.is_empty())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    parameters.id = id.to_owned();
    let Ok(route) = route(state, target).await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if session.is_some_and(|session| current(state, session).is_err())
        || peer_address::current(&state.db, target, &route.peer).is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(url) = super::resource_url(&route.base, "fs", &parameters) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    relay(route.client.get(url), headers, false).await
}

async fn route(state: &ServerState, target: &str) -> Result<Arc<Route>> {
    let peer = peers::get(&state.db, target)?
        .filter(|peer| peer.is_paired() && !peer.public_key.is_empty())
        .ok_or_else(|| anyhow!("Attachment peer unavailable"))?;
    let slot = state
        .managed_proxy
        .attachments
        .lock()
        .unwrap()
        .entry(target.to_owned())
        .or_default()
        .clone();
    let mut cached = slot.lock().await;
    if let Some(route) = cached.as_ref().filter(|route| {
        route.expires > Instant::now()
            && route.peer.ip == peer.ip
            && route.peer.port == peer.port
            && peer_address::same_identity(&route.peer, &peer)
    }) {
        return Ok(route.clone());
    }
    let base = http_client::Url::parse(&peer_address::address(&peer).base_url)?;
    let response = state
        .lan
        .client
        .post(base.join("/init")?)
        .header(
            "c-id",
            state.native_client_id().map_err(anyhow::Error::msg)?,
        )
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    let status = response.status();
    let pin = if status.is_success() {
        let bytes = peer_lan::limited(response, 8192).await?;
        let reply: serde_json::Value = serde_json::from_slice(&bytes)?;
        match (
            reply["tlsCertificateSha256"].as_str(),
            reply["tlsCertificateSignature"].as_str(),
        ) {
            (Some(pin), Some(signature)) => {
                let proof = format!("plain-peer-tls-v1:{pin}");
                ensure!(
                    plain_libs::crypto::ed25519_verify(
                        &peer.public_key,
                        proof.as_bytes(),
                        signature
                    ),
                    "Invalid peer TLS signature"
                );
                Some(decode_pin(pin)?)
            }
            (None, None) => None,
            _ => return Err(anyhow!("Incomplete peer TLS proof")),
        }
    } else {
        None
    };
    let client = if let Some(route) = cached
        .as_ref()
        .filter(|route| route.base == base && route.pin == pin)
    {
        route.client.clone()
    } else {
        let mut builder = http_client::builder()
            .no_proxy()
            .https_only(true)
            .redirect(http_client::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(8);
        if let Some(pin) = pin {
            builder = builder.pin_certificate(pin);
        }
        builder.build()?
    };
    let route = Arc::new(Route {
        peer,
        base,
        pin,
        client,
        expires: Instant::now() + ROUTE_LIFETIME,
    });
    *cached = Some(route.clone());
    Ok(route)
}
