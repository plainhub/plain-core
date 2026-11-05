use plain_rs::features::access::AccessFeature;
use plain_graphql::Context;
use plain_rs::features::prefs::Prefs;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

pub(crate) fn is_local(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        .is_some_and(|origin| *origin == crate::graphql::executor::RequestOrigin::Local)
}

pub(crate) struct LocalOwner;

impl plain_graphql::Guard for LocalOwner {
    async fn check(&self, ctx: &Context<'_>) -> plain_graphql::Result<()> {
        if is_local(ctx) {
            Ok(())
        } else {
            Err(plain_graphql::Error::new("no_permission"))
        }
    }
}

pub(crate) fn require(
    ctx: &plain_graphql::Context<'_>,
    prefs: &Prefs,
    features: &[AccessFeature],
) -> plain_graphql::Result<()> {
    if is_local(ctx) {
        return Ok(());
    }
    if plain_rs::features::services::device::access::is_enabled(prefs, features) {
        Ok(())
    } else {
        Err(plain_graphql::Error::new("no_permission"))
    }
}

pub(crate) async fn require_granted(
    ctx: &Context<'_>,
    features: &[AccessFeature],
) -> anyhow::Result<bool> {
    if !is_local(ctx)
        && !plain_rs::features::services::device::access::is_enabled(
            ctx.data_unchecked::<Arc<Prefs>>(),
            features,
        )
    {
        return Ok(false);
    }
    plain_rs::features::services::device::access::granted(
        ctx.data_unchecked::<Arc<Host>>(),
        features,
    )
    .await
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/http/auth/public_gate.rs"]
mod tests;
