pub mod mutation;
pub mod query;

use crate::server::ServerState;
use plain_graphql::{Context, SimpleObject};
use plain_rs::infrastructure::types::{Instant, Long};
use serde::Deserialize;

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MdnsPacket {
    time: Instant,
    direction: plain_rs::infrastructure::protocols::mdns::packet_capture::MdnsPacketDirection,
    src_ip: String,
    src_port: i32,
    dst_ip: String,
    dst_port: i32,
    size: Long,
    summary: String,
    detail: String,
}

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MdnsService {
    service_type: String,
    instance_name: String,
    instance_fqdn: String,
    hostname: String,
    port: i32,
    txt_records: Vec<String>,
    ips: Vec<String>,
    ip6s: Vec<String>,
    complete: bool,
}

#[derive(SimpleObject, Deserialize)]
#[graphql(complex)]
#[serde(rename_all = "camelCase")]
struct MdnsState {
    runtime_id: String,
    packets_in: Vec<MdnsPacket>,
    packets_out: Vec<MdnsPacket>,
    revision: Long,
    running: bool,
    scanning: bool,
    services: Vec<MdnsService>,
}

#[derive(SimpleObject)]
struct MdnsActivity {
    time: Instant,
    event: String,
    detail: String,
}

#[plain_graphql::ComplexObject]
impl MdnsState {
    async fn activity(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<MdnsActivity>> {
        if !crate::http::auth::public_gate::is_local(ctx) {
            crate::graphql::resolvers::app::http_server::authorized_capabilities(ctx).await?;
        }
        let runtime = &ctx.data::<ServerState>()?.mdns;
        let activity = runtime
            .browser_activity()
            .into_iter()
            .map(|entry| {
                Ok(MdnsActivity {
                    time: crate::graphql::types::instant_from_millis(i64::try_from(entry.time)?)?,
                    event: entry.event,
                    detail: entry.detail,
                })
            })
            .collect::<plain_graphql::Result<Vec<_>>>()?;
        Ok(activity)
    }
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/discovery/mdns/mod.rs"]
mod tests;
