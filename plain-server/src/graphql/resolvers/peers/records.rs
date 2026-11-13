#[derive(Default)]
pub struct PeerRecordQuery;
#[plain_graphql::Object]
impl PeerRecordQuery {
    async fn peer_file_url(
        &self,
        ctx: &plain_graphql::Context<'_>,
        peer_id: plain_graphql::ID,
        file_id: String,
    ) -> plain_graphql::Result<String> {
        let db = ctx.data::<crate::server::ServerState>()?.db.clone();
        tokio::task::spawn_blocking(move || {
            let peer = plain_rs::infrastructure::db::chat_store::peers::get(&db, &peer_id)?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Peer removed"))?;
            plain_rs::features::services::peers::peer_address::download_url(&peer, &file_id)
        })
        .await?
        .map_err(|error| plain_graphql::Error::new(error.to_string()))
    }
}
#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/peers/records.rs"]
mod tests;
