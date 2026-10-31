use super::types::NearbyDevice;
use crate::server::ServerState;
use async_graphql::{Context, Object};
use std::collections::HashSet;

#[derive(Default)]
pub struct DiscoveryHostQuery;

#[Object]
impl DiscoveryHostQuery {
    async fn parse_pairing_code(&self, code: String) -> Option<NearbyDevice> {
        plain_rs::discovery::pairing_code::parse(&code)
            .map(|payload| NearbyDevice::from_snapshot_device(payload.into_device()))
    }
    async fn nearby_devices(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<NearbyDevice>> {
        let state = ctx.data::<ServerState>()?;
        nearby_snapshot(state)
    }
}

fn nearby_snapshot(state: &ServerState) -> async_graphql::Result<Vec<NearbyDevice>> {
    let paired: HashSet<String> = plain_rs::db::chat_store::peers::all(&state.db)?
        .into_iter()
        .filter(|peer| peer.is_paired())
        .map(|peer| peer.id)
        .collect();
    Ok(state
        .nearby_devices
        .snapshot()
        .devices
        .into_iter()
        .map(|device| {
            let mut model = NearbyDevice::from_snapshot_device(device);
            if paired.contains(model.id.as_str()) {
                model.status = "PAIRED".to_owned();
            }
            model
        })
        .collect())
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/discovery/host.rs"]
mod tests;
