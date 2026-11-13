pub const MANAGED_RESOURCE: &str = "x-plain-managed-resource";

use crate::runtime::Runtime as CoreRuntime;
use crate::{
    features::services::peers::{management_http, peer_address},
    infrastructure::db::{DPeer, chat_store::peers},
};
use plain_libs::anyhow::{Result, anyhow, ensure};
use plain_libs::protocols::http_client;
use plain_libs::serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const GRAPHQL_LIMIT: usize = 4 * 1024 * 1024;
pub const ROUTE_LIFETIME: Duration = Duration::from_secs(30);
type RouteSlot = Arc<tokio::sync::Mutex<Option<Arc<PeerRoute>>>>;

#[derive(Default)]
pub struct ManagedPeerRoutes {
    #[cfg(feature = "test-seams")]
    pub routes: Mutex<HashMap<String, RouteSlot>>,
    #[cfg(not(feature = "test-seams"))]
    routes: Mutex<HashMap<String, RouteSlot>>,
}

pub struct PeerRoute {
    pub peer: DPeer,
    pub base: http_client::Url,
    pub client_id: String,
    pub key: Vec<u8>,
    pub url_key: Vec<u8>,
    pub pin: [u8; 32],
    pub client: http_client::Client,
    pub expires: Instant,
}

impl ManagedPeerRoutes {
    pub async fn route(&self, state: &CoreRuntime, target: &str) -> Result<Arc<PeerRoute>> {
        let peer = peers::get(&state.db, target)?
            .filter(|peer| {
                peer.is_paired() && !peer.public_key.is_empty() && !peer.token.is_empty()
            })
            .ok_or_else(|| anyhow!("Managed device unavailable"))?;
        let client_id = state
            .native_client_id()
            .map_err(plain_libs::anyhow::Error::msg)?;
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
        let base = http_client::Url::parse(
            &peer
                .base_url()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("No peer address"))?,
        )?;
        let key = decode_key(&peer.token)?;
        let data = management_http::query(
            state,
            &base,
            &client_id,
            &key,
            json!({"query":"{ app { clientId urlToken tlsCertificateSha256 } }"}),
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
            expires: Instant::now() + ROUTE_LIFETIME,
        });
        route.check_current(state)?;
        *cached = Some(route.clone());
        Ok(route)
    }
}

impl PeerRoute {
    pub async fn exchange(
        &self,
        state: &CoreRuntime,
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

    pub fn check_current(&self, state: &CoreRuntime) -> Result<()> {
        check_current(state, &self.peer)
    }
}

fn check_current(state: &CoreRuntime, expected: &DPeer) -> Result<()> {
    let peer = peer_address::current(&state.db, &expected.id, expected)?;
    ensure!(
        peer.is_paired() && peer.token == expected.token,
        "Management session revoked"
    );
    Ok(())
}

pub fn decode_key(token: &str) -> Result<Vec<u8>> {
    let key = plain_libs::utils::base64::base64_decode_checked(token)?;
    ensure!(key.len() == 32, "Invalid session key");
    Ok(key)
}

pub fn decode_pin(hex: &str) -> Result<[u8; 32]> {
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
