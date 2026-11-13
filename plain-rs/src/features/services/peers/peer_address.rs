use crate::infrastructure::db::DPeer;
use plain_libs::protocols::http_client;
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
