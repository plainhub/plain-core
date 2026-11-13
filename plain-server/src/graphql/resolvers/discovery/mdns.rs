use crate::server::ServerState;
use plain_graphql::{Context, Object, SimpleObject};

use plain_rs::features::services::discovery::mdns_runtime;
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
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    fn activity(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<MdnsActivity>> {
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

#[derive(Default)]
pub struct MdnsQuery;
#[Object]
impl MdnsQuery {
    fn is_discovering(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        let facts = ctx.data::<ServerState>()?.mdns.snapshot();
        Ok(facts["scanning"].as_bool().unwrap_or(false))
    }
    fn mdns_state(&self, ctx: &Context<'_>) -> plain_graphql::Result<MdnsState> {
        let state = ctx.data::<ServerState>()?;
        if *state.stop.borrow() {
            return Err("Server stopped".into());
        }
        Ok(plain_libs::serde_json::from_value(
            state.mdns.snapshot_fields(
                ctx.look_ahead().field("services").exists(),
                ctx.look_ahead().field("packetsIn").exists()
                    || ctx.look_ahead().field("packetsOut").exists(),
            ),
        )?)
    }
}
#[derive(Default)]
pub struct MdnsMutation;
#[Object]
impl MdnsMutation {
    async fn start_discovery(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        mdns_runtime::control(
            ctx.data::<ServerState>()?,
            mdns_runtime::Request::Start {},
        )
        .await?;
        Ok(true)
    }

    async fn stop_discovery(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        mdns_runtime::control(
            ctx.data::<ServerState>()?,
            mdns_runtime::Request::Stop {},
        )
        .await?;
        Ok(true)
    }
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/discovery/mdns.rs"]
mod tests;
