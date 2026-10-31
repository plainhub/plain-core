use plain_rs::db::DPeer;
use serde::{Serialize, Serializer};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub best_ip: String,
    pub name: String,
    pub base_url: String,
    pub api_url: String,
    pub status_ws_url: String,
}
#[derive(Serialize)]
pub struct View<'a> {
    #[serde(flatten)]
    peer: &'a DPeer,
    address: Address,
}
pub fn address(peer: &DPeer) -> Address {
    address_with_interfaces(peer, &plain_rs::transport::lan::local_interfaces())
}
pub fn address_with_interfaces(
    peer: &DPeer,
    local: &[plain_rs::transport::lan::Interface],
) -> Address {
    let ip = peer.best_ip_with_interfaces(local);
    let base_url = peer
        .port
        .filter(|port| *port > 0)
        .filter(|_| !ip.is_empty())
        .map(|port| plain_rs::utils::http_url::build_url("https", &ip, port, ""))
        .unwrap_or_default();
    Address {
        name: if peer.name.trim().is_empty() {
            ip.clone()
        } else {
            peer.name.clone()
        },
        api_url: if base_url.is_empty() {
            String::new()
        } else {
            format!("{base_url}/peer_graphql")
        },
        status_ws_url: if base_url.is_empty() {
            String::new()
        } else {
            base_url.replacen("https", "wss", 1) + "/status"
        },
        best_ip: ip,
        base_url,
    }
}
pub fn view(peer: &DPeer) -> View<'_> {
    View {
        peer,
        address: address(peer),
    }
}
pub fn serialize_view<S: Serializer>(peer: &DPeer, serializer: S) -> Result<S::Ok, S::Error> {
    view(peer).serialize(serializer)
}
pub fn current(db: &plain_rs::db::Db, id: &str, expected: &DPeer) -> anyhow::Result<DPeer> {
    let peer = current_identity(db, id, expected)?;
    anyhow::ensure!(
        peer.ip == expected.ip && peer.port == expected.port,
        "Peer changed"
    );
    Ok(peer)
}
pub fn current_identity(
    db: &plain_rs::db::Db,
    id: &str,
    expected: &DPeer,
) -> anyhow::Result<DPeer> {
    let peer = plain_rs::db::chat_store::peers::get(db, id)?
        .ok_or_else(|| anyhow::anyhow!("Peer removed"))?;
    anyhow::ensure!(same_identity(&peer, expected), "Peer changed");
    Ok(peer)
}
pub fn same_identity(peer: &DPeer, expected: &DPeer) -> bool {
    peer.id == expected.id
        && peer.key == expected.key
        && peer.public_key == expected.public_key
        && peer.status == expected.status
}
pub fn file_url(peer: &DPeer, file_id: &str) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(&format!(
        "{}/fs",
        peer.base_url()
            .ok_or_else(|| anyhow::anyhow!("No peer address"))?
    ))?;
    url.query_pairs_mut().append_pair("id", file_id);
    Ok(url.to_string())
}
pub fn proxy_address(db: &plain_rs::db::Db, id: &str) -> anyhow::Result<Option<String>> {
    let peer = plain_rs::db::chat_store::peers::get(db, id)?
        .filter(|peer| peer.is_paired() || !peer.token.is_empty());
    Ok(peer.and_then(|peer| {
        if peer.ip.is_empty() {
            return None;
        }
        Some(format!("{}:{}", peer.best_ip(), peer.port?))
    }))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/peer_address.rs"]
mod tests;
