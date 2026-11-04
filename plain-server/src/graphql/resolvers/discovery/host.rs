use super::types::NearbyDevice;
use crate::server::ServerState;
use plain_graphql::{Context, Object};

use std::collections::HashSet;

#[derive(Default)]
pub struct DiscoveryHostQuery;

#[Object]
impl DiscoveryHostQuery {
    fn parse_pairing_code(&self, code: String) -> Option<NearbyDevice> {
        plain_rs::features::discovery::pairing_code::parse(&code)
            .map(|payload| NearbyDevice::from_snapshot_device(payload.into_device()))
    }
    fn nearby_devices(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<NearbyDevice>> {
        let state = ctx.data::<ServerState>()?;
        nearby_snapshot(state)
    }
}

fn nearby_snapshot(state: &ServerState) -> plain_graphql::Result<Vec<NearbyDevice>> {
    let paired: HashSet<String> = plain_rs::infrastructure::db::chat_store::peers::all(&state.db)?
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
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/discovery/host.rs"]
mod tests;
