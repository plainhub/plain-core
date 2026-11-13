use plain_libs::protocols::http_client;
use crate::infrastructure::db::DPeer;
use serde::{Serialize, Serializer};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub best_ip: String,
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
    address_with_interfaces(
        peer,
        &crate::infrastructure::transport::lan::local_interfaces(),
    )
}
pub fn address_with_interfaces(
    peer: &DPeer,
    local: &[crate::infrastructure::transport::lan::Interface],
) -> Address {
    let ip = peer.best_ip_with_interfaces(local);
    let base_url = peer
        .port
        .filter(|port| *port > 0)
        .filter(|_| !ip.is_empty())
        .map(|port| plain_libs::utils::http_url::build_url("https", &ip, port, ""))
        .unwrap_or_default();
    Address {
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
pub fn current(
    db: &crate::infrastructure::db::Db,
    id: &str,
    expected: &DPeer,
) -> plain_libs::anyhow::Result<DPeer> {
    let peer = current_identity(db, id, expected)?;
    plain_libs::anyhow::ensure!(
        peer.ip == expected.ip && peer.port == expected.port,
        "Peer changed"
    );
    Ok(peer)
}
pub fn current_identity(
    db: &crate::infrastructure::db::Db,
    id: &str,
    expected: &DPeer,
) -> plain_libs::anyhow::Result<DPeer> {
    let peer = crate::infrastructure::db::chat_store::peers::get(db, id)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Peer removed"))?;
    plain_libs::anyhow::ensure!(same_identity(&peer, expected), "Peer changed");
    Ok(peer)
}
pub fn same_identity(peer: &DPeer, expected: &DPeer) -> bool {
    peer.id == expected.id
        && peer.key == expected.key
        && peer.public_key == expected.public_key
        && peer.status == expected.status
}
pub fn download_url(peer: &DPeer, file_id: &str) -> plain_libs::anyhow::Result<String> {
    let mut url = http_client::Url::parse(&format!(
        "{}/fs",
        peer.base_url()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("No peer address"))?
    ))?;
    url.query_pairs_mut()
        .append_pair("id", file_id)
        .append_pair(
            "action",
            crate::features::services::files::content::Action::Download.as_str(),
        );
    Ok(url.to_string())
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/peer_address.rs"]
mod tests;
