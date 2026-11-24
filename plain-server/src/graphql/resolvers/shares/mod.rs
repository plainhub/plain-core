pub mod mutation;
pub mod query;

use crate::graphql::types::parse_instant;
use crate::server::ServerState;
use plain_graphql::{Context, ID, SimpleObject};
use plain_rs::features::files::shares::{Service, SharePath};
use plain_rs::features::services::peers::shared_client;
use plain_rs::infrastructure::db::ShareRow;
use plain_rs::infrastructure::types::Instant;

#[derive(SimpleObject)]
struct Share {
    id: ID,
    name: String,
    password: String,
    expires_at: Option<Instant>,
    read_only: bool,
    data: Vec<SharePath>,
    url: String,
}

fn state<'a>(ctx: &'a Context<'_>) -> plain_graphql::Result<&'a ServerState> {
    let state = ctx.data::<ServerState>()?;
    crate::http::auth::public_gate::require(
        ctx,
        &state.prefs,
        &[plain_rs::features::access::AccessFeature::Files],
    )?;
    Ok(state)
}

async fn record(state: &ServerState, row: ShareRow) -> plain_graphql::Result<Share> {
    let data = Service::roots(&row)?;
    let url = shared_client::own_link(state, &row.id).await?.page_url;
    Ok(Share {
        id: row.id.into(),
        name: row.name,
        password: row.password,
        expires_at: row.expires_at.as_deref().map(parse_instant).transpose()?,
        read_only: row.read_only,
        data,
        url,
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/shares/mod.rs"]
mod tests;
