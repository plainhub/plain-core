pub mod mutation;
pub mod query;

use plain_graphql::{Context, Result};
use plain_rs::features::capability::Capability;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

async fn require_capability(ctx: &Context<'_>, name: Capability) -> Result<()> {
    plain_rs::features::services::capabilities::require(ctx.data::<Arc<Host>>()?, name)
        .await
        .map_err(|error| plain_graphql::Error::from(error))
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/app/platform/mod.rs"]
mod tests;
