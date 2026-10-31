use async_graphql::SimpleObject;
use plain_rs::types::Instant;
use serde::Deserialize;

#[derive(SimpleObject, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PeerAddress {
    best_ip: String,
    name: String,
    base_url: String,
    api_url: String,
    status_ws_url: String,
}
impl From<plain_rs::services::peers::peer_address::Address> for PeerAddress {
    fn from(value: plain_rs::services::peers::peer_address::Address) -> Self {
        Self {
            best_ip: value.best_ip,
            name: value.name,
            base_url: value.base_url,
            api_url: value.api_url,
            status_ws_url: value.status_ws_url,
        }
    }
}
#[derive(async_graphql::InputObject, serde::Serialize)]
struct PeerRecordInput {
    id: async_graphql::ID,
    name: String,
    ip: String,
    key: String,
    public_key: String,
    status: String,
    port: Option<i32>,
    device_type: String,
    token: String,
    created_at: Instant,
    updated_at: Instant,
}
#[derive(Default)]
pub struct PeerRecordQuery;
#[async_graphql::Object]
impl PeerRecordQuery {
    async fn peer_file_url(
        &self,
        ctx: &async_graphql::Context<'_>,
        id: async_graphql::ID,
        expected: PeerRecordInput,
        file_id: String,
    ) -> async_graphql::Result<String> {
        let db = ctx.data::<crate::server::ServerState>()?.db.clone();
        let expected = serde_json::from_value(serde_json::to_value(expected)?)?;
        tokio::task::spawn_blocking(move || {
            let peer = plain_rs::services::peers::peer_address::current(&db, &id, &expected)?;
            plain_rs::services::peers::peer_address::file_url(&peer, &file_id)
        })
        .await?
        .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}
#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../tests/unit/graphql/resolvers/peers/records.rs"]
mod tests;
