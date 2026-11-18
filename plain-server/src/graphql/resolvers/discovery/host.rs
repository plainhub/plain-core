use super::types::NearbyDevice;
use crate::server::ServerState;
use plain_graphql::{Context, Object};
use plain_rs::infrastructure::transport::lan;

use std::collections::HashSet;

#[derive(Default)]
pub struct DiscoveryHostQuery;

#[Object]
impl DiscoveryHostQuery {
    fn parse_pairing_qr_code(&self, ctx: &Context<'_>, code: String) -> Option<NearbyDevice> {
        ctx.data::<ServerState>().ok()?.pairing_runtime.qr.remember(&code)
            .ok().map(|device| {
                let best_ip = lan::best(&device.ips, &lan::local_interfaces());
                NearbyDevice::from_snapshot_device(device, best_ip)
            })
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
        .visible_devices(&lan::local_interfaces())
        .into_iter()
        .map(|(device, best_ip)| {
            let mut model = NearbyDevice::from_snapshot_device(device, best_ip);
            if paired.contains(model.id.as_str()) {
                model.status = plain_rs::features::chat::enums::PeerStatus::Paired;
            }
            model
        })
        .collect())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/discovery/host.rs"]
mod tests;
