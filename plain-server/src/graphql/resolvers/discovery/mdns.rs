use crate::server::ServerState;
use async_graphql::{Context, Object, SimpleObject};
use plain_rs::services::discovery::mdns_runtime;
use plain_rs::services::discovery::mdns_runtime::Request;
use plain_rs::types::Long;
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

#[derive(Default, SimpleObject, Deserialize)]
#[graphql(complex)]
#[serde(rename_all = "camelCase")]
struct MdnsState {
    runtime_id: String,
    packets_in: Vec<MdnsPacket>,
    packets_out: Vec<MdnsPacket>,
    revision: Long,
    receiver: bool,
    running: bool,
    scanning: bool,
    published: bool,
    manual_scanning: bool,
    debug_session_count: i32,
    capturing: bool,
    services: Vec<MdnsService>,
}

#[derive(SimpleObject)]
struct MdnsActivity {
    time: Long,
    event: String,
    detail: String,
}
#[async_graphql::ComplexObject]
impl MdnsState {
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    async fn activity(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<MdnsActivity>> {
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
            .collect::<async_graphql::Result<Vec<_>>>()?;
        Ok(activity)
    }
}

async fn control(ctx: &Context<'_>, request: Request) -> async_graphql::Result<MdnsState> {
    let value = mdns_runtime::control(ctx.data::<ServerState>()?, request)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(serde_json::from_value(value)?)
}
#[derive(Default)]
pub struct MdnsQuery;
#[Object]
impl MdnsQuery {
    async fn mdns_state(&self, ctx: &Context<'_>) -> async_graphql::Result<MdnsState> {
        let state = ctx.data::<ServerState>()?;
        if *state.stop.borrow() {
            return Err("Server stopped".into());
        }
        if ctx
            .field()
            .selection_set()
            .all(|field| matches!(field.name(), "services" | "activity" | "__typename"))
        {
            let services = if ctx.look_ahead().field("services").exists() {
                serde_json::from_value(serde_json::to_value(state.mdns.browser_snapshot())?)?
            } else {
                Vec::new()
            };
            return Ok(MdnsState {
                services,
                ..Default::default()
            });
        }
        control(ctx, Request::Snapshot {}).await
    }
}
#[derive(Default)]
pub struct MdnsMutation;
#[Object]
impl MdnsMutation {
    async fn acquire_discovery(
        &self,
        ctx: &Context<'_>,
        token: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
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
        token: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
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
#[path = "../../../../tests/unit/graphql/resolvers/discovery/mdns.rs"]
mod tests;

fn discovery_owner(ctx: &Context<'_>) -> async_graphql::Result<String> {
    if crate::http::auth::public_gate::is_local(ctx) {
        return Ok(ctx
            .data_opt::<plain_rs::host::request_scope::NativeRequestScope>()
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
