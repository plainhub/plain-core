pub mod mutation;
pub mod query;

use crate::graphql::executor::RequestClient;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Result, SimpleObject};
use plain_rs::features::access_error::AccessErrorCode;
use plain_rs::features::capability::Capability;
use plain_rs::features::services::http_status::HttpServerState;

pub(crate) async fn authorized_capabilities(ctx: &Context<'_>) -> Result<Vec<Capability>> {
    if ctx.data_opt::<RequestClient>().is_none() {
        return Err(AccessErrorCode::NoPermission.into());
    }
    let capabilities =
        plain_rs::features::services::capabilities::snapshot(&ctx.data::<ServerState>()?.host)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
    plain_rs::features::services::capabilities::require_list(&capabilities, Capability::HttpServer)
        .map_err(|error| plain_graphql::Error::from(error))?;
    Ok(capabilities)
}

#[derive(SimpleObject, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpServer {
    state: HttpServerState,
    addresses: Vec<String>,
    online_clients: Vec<ID>,
    remote_access: bool,
    mdns_hostname: String,
}

pub(crate) async fn snapshot(state: &ServerState) -> Result<HttpServer> {
    Ok(HttpServer {
        state: state.http_server.snapshot(),
        remote_access: plain_rs::features::prefs::settings::snapshot(&state.prefs)?.remote_access,
        mdns_hostname: state
            .prefs
            .get::<String>(plain_rs::features::prefs::pref_keys::MDNS_HOSTNAME)?
            .ok_or_else(|| plain_graphql::Error::new("mDNS hostname is unavailable"))?,
        addresses: plain_rs::infrastructure::platform::network::host::lan_ipv4_addresses(
            &state.host,
        )
        .await?,
        online_clients: state
            .subscription_sessions
            .snapshot()
            .into_iter()
            .map(ID)
            .collect(),
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/app/http_server/mod.rs"]
mod tests;
