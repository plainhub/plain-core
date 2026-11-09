pub(crate) mod upload;

use crate::server::ServerState;
use anyhow::{Result, anyhow, ensure};
use plain_libs::protocols::http_client;
use plain_rs::{
    features::services::peers::{management_http, peer_address},
    infrastructure::db::{DPeer, chat_store::peers},
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) const GRAPHQL_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const ROUTE_LIFETIME: Duration = Duration::from_secs(30);
type RouteSlot = Arc<tokio::sync::Mutex<Option<Arc<PeerRoute>>>>;

#[derive(Default)]
pub(crate) struct ManagedPeerRoutes {
    routes: Mutex<HashMap<String, RouteSlot>>,
}

pub(crate) struct PeerRoute {
    pub(crate) peer: DPeer,
    pub(crate) base: http_client::Url,
    pub(crate) client_id: String,
    pub(crate) key: Vec<u8>,
    pub(crate) url_key: Vec<u8>,
    pub(crate) pin: [u8; 32],
    pub(crate) client: http_client::Client,
    pub(crate) supports_managed_resources: bool,
    pub(crate) expires: Instant,
}

impl ManagedPeerRoutes {
    pub(crate) async fn route(&self, state: &ServerState, target: &str) -> Result<Arc<PeerRoute>> {
        let peer = peers::get(&state.db, target)?
            .filter(|peer| {
                peer.is_paired() && !peer.public_key.is_empty() && !peer.token.is_empty()
            })
            .ok_or_else(|| anyhow!("Managed device unavailable"))?;
        let client_id = state.native_client_id().map_err(anyhow::Error::msg)?;
        ensure!(target != client_id, "Invalid managed target");
        let slot = self
            .routes
            .lock()
            .unwrap()
            .entry(target.into())
            .or_default()
            .clone();
        let mut cached = slot.lock().await;
        if let Some(route) = cached.as_ref().filter(|route| {
            route.expires > Instant::now()
                && route.peer.ip == peer.ip
                && route.peer.port == peer.port
                && route.peer.token == peer.token
                && peer_address::same_identity(&route.peer, &peer)
        }) {
            return Ok(route.clone());
        }
        let base = http_client::Url::parse(&peer_address::address(&peer).base_url)?;
        let key = decode_key(&peer.token)?;
        let data = management_http::query(
            state,
            &base,
            &client_id,
            &key,
            json!({"query":"{ app { clientId urlToken tlsCertificateSha256 } __type(name: \"Peer\") { fields { name } } }"}),
            GRAPHQL_LIMIT,
        )
        .await;
        check_current(state, &peer)?;
        let data = data?;
        let app = &data["app"];
        ensure!(
            app["clientId"].as_str() == Some(target),
            "Managed device identity changed"
        );
        let url_key = decode_key(app["urlToken"].as_str().unwrap_or_default())?;
        let pin = decode_pin(app["tlsCertificateSha256"].as_str().unwrap_or_default())?;
        let supports_managed_resources = data["__type"]["fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| field["name"] == "managed"));
        let client = if let Some(route) = cached
            .as_ref()
            .filter(|route| route.base == base && route.pin == pin)
        {
            route.client.clone()
        } else {
            http_client::builder()
                .no_proxy()
                .https_only(true)
                .pin_certificate(pin)
                .redirect(http_client::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .read_timeout(Duration::from_secs(30))
                .pool_max_idle_per_host(8)
                .build()?
        };
        let route = Arc::new(PeerRoute {
            peer,
            base,
            client_id,
            key,
            url_key,
            pin,
            client,
            supports_managed_resources,
            expires: Instant::now() + ROUTE_LIFETIME,
        });
        route.check_current(state)?;
        *cached = Some(route.clone());
        Ok(route)
    }
}

impl PeerRoute {
    pub(crate) async fn exchange(
        &self,
        state: &ServerState,
        request: &str,
        budget: Option<Duration>,
    ) -> Result<Vec<u8>> {
        ensure!(request.len() <= GRAPHQL_LIMIT, "GraphQL request too large");
        self.check_current(state)?;
        let outgoing = self
            .client
            .post(self.base.join("/graphql")?)
            .header("c-id", &self.client_id);
        let outgoing = match budget {
            Some(remaining) => outgoing.timeout(remaining).read_timeout(Some(remaining)),
            None => outgoing.timeout(Duration::from_secs(10)),
        };
        let response = management_http::exchange(outgoing, &self.key, request, GRAPHQL_LIMIT).await;
        self.check_current(state)?;
        response
    }

    pub(crate) fn check_current(&self, state: &ServerState) -> Result<()> {
        check_current(state, &self.peer)
    }
}

fn check_current(state: &ServerState, expected: &DPeer) -> Result<()> {
    let peer = peer_address::current(&state.db, &expected.id, expected)?;
    ensure!(
        peer.is_paired() && peer.token == expected.token,
        "Management session revoked"
    );
    Ok(())
}

pub(crate) fn decode_key(token: &str) -> Result<Vec<u8>> {
    let key = plain_libs::utils::base64::base64_decode_checked(token)?;
    ensure!(key.len() == 32, "Invalid session key");
    Ok(key)
}

pub(crate) fn decode_pin(hex: &str) -> Result<[u8; 32]> {
    ensure!(
        hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid certificate fingerprint"
    );
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)?;
    }
    Ok(digest)
}

pub(crate) fn request_error(error: anyhow::Error) -> String {
    use plain_rs::features::services::peers::management_http::AuthorizationError;
    if error
        .downcast_ref::<http_client::Error>()
        .is_some_and(http_client::Error::is_timeout)
    {
        return "connection_timeout".into();
    }
    match error.downcast_ref::<AuthorizationError>() {
        Some(AuthorizationError::Session) => "unauthorized".into(),
        Some(AuthorizationError::Permission) => "remote_access_disabled".into(),
        None => error.to_string(),
    }
}

#[cfg(test)]
#[path = "../../tests/plain-server/unit/managed_peers.rs"]
mod tests;
