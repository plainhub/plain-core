use crate::server::ServerState;
use plain_graphql::{Context, Object, SimpleObject};

use plain_rs::features::services::discovery::mdns_runtime;
use plain_rs::features::services::discovery::mdns_runtime::Request;
use plain_rs::infrastructure::types::Long;
use serde::Deserialize;

#[derive(SimpleObject, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MdnsPacket {
    time: Long,
    direction: String,
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
    ipv6: Vec<String>,
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
    time: Long,
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
                    time: Long(i64::try_from(entry.time)?),
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
    fn mdns_state(&self, ctx: &Context<'_>) -> plain_graphql::Result<MdnsState> {
        let state = ctx.data::<ServerState>()?;
        if *state.stop.borrow() {
            return Err("Server stopped".into());
        }
        Ok(serde_json::from_value(state.mdns.snapshot_fields(
            ctx.look_ahead().field("services").exists(),
            ctx.look_ahead().field("packetsIn").exists()
                || ctx.look_ahead().field("packetsOut").exists(),
        ))?)
    }
}
#[derive(Default)]
pub struct MdnsMutation;
#[Object]
impl MdnsMutation {
    async fn acquire_discovery(
        &self,
        ctx: &Context<'_>,
        token: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        mdns_runtime::control(
            ctx.data::<ServerState>()?,
            Request::Acquire {
                token: uuid::Uuid::parse_str(&token)?,
                owner: discovery_owner(ctx)?,
            },
        )
        .await?;
        Ok(true)
    }
    async fn release_discovery(
        &self,
        ctx: &Context<'_>,
        token: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        mdns_runtime::control(
            ctx.data::<ServerState>()?,
            Request::Release {
                token: uuid::Uuid::parse_str(&token)?,
                owner: discovery_owner(ctx)?,
            },
        )
        .await?;
        Ok(true)
    }
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/discovery/mdns.rs"]
mod tests;

fn discovery_owner(ctx: &Context<'_>) -> plain_graphql::Result<String> {
    if crate::http::auth::public_gate::is_local(ctx) {
        return Ok(ctx
            .data_opt::<plain_rs::infrastructure::host::request_scope::NativeRequestScope>()
            .map(|scope| format!("native:{}", scope.id()))
            .unwrap_or_else(|| "native".into()));
    }
    let client = ctx
        .data::<crate::graphql::executor::RequestClient>()?
        .client_id();
    if client.is_empty() {
        return Err("Missing discovery client".into());
    }
    Ok(mdns_runtime::remote_discovery_owner(client))
}
