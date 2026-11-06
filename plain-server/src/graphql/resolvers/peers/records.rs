use plain_graphql::SimpleObject;
use plain_rs::infrastructure::types::Instant;
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
impl From<plain_rs::features::services::peers::peer_address::Address> for PeerAddress {
    fn from(value: plain_rs::features::services::peers::peer_address::Address) -> Self {
        Self {
            best_ip: value.best_ip,
            name: value.name,
            base_url: value.base_url,
            api_url: value.api_url,
            status_ws_url: value.status_ws_url,
        }
    }
}
#[derive(plain_graphql::InputObject, serde::Serialize)]
struct PeerRecordInput {
    id: plain_graphql::ID,
    name: String,
    ip: String,
    key: String,
    public_key: String,
    status: plain_rs::features::chat::enums::PeerStatus,
    port: Option<i32>,
    device_type: plain_rs::features::services::device::types::DeviceType,
    token: String,
    created_at: Instant,
    updated_at: Instant,
}
#[derive(Default)]
pub struct PeerRecordQuery;
#[plain_graphql::Object]
impl PeerRecordQuery {
    async fn peer_file_url(
        &self,
        ctx: &plain_graphql::Context<'_>,
        id: plain_graphql::ID,
        expected: PeerRecordInput,
        file_id: String,
    ) -> plain_graphql::Result<String> {
        let db = ctx.data::<crate::server::ServerState>()?.db.clone();
        let expected = serde_json::from_value(serde_json::to_value(expected)?)?;
        tokio::task::spawn_blocking(move || {
            let peer =
                plain_rs::features::services::peers::peer_address::current(&db, &id, &expected)?;
            plain_rs::features::services::peers::peer_address::download_url(&peer, &file_id)
        })
        .await?
        .map_err(|error| plain_graphql::Error::new(error.to_string()))
    }
}
#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/peers/records.rs"]
mod tests;
