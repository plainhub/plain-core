use super::Peer;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::Value;

#[derive(Default)]
pub struct PeersQuery;

#[Object]
impl PeersQuery {
    fn peers(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Peer>> {
        let state = ctx.data::<crate::server::ServerState>()?;
        let rows = plain_rs::infrastructure::db::chat_store::peers::all(&state.db)?;
        let connections = state.peer_status.connections.snapshot_peers(&rows);
        let online = connections["online"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<std::collections::HashSet<_>>();
        Ok(rows
            .into_iter()
            .map(|row| {
                let connected = online.contains(row.id.as_str());
                Peer::from_record(row, connected)
            })
            .collect())
    }
}
