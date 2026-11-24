//! Public preference roots share the local Rust store and notify UI projections.

pub mod mutation;
pub mod state;

pub mod query;

use plain_graphql::Context;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::prefs::pref_keys::{PrefAccess, access};
use std::sync::Arc;

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<Prefs> {
    ctx.data_unchecked::<Arc<Prefs>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/settings/mod.rs"]
mod tests;

fn is_local(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        == Some(&crate::graphql::executor::RequestOrigin::Local)
}

pub(crate) async fn authorize(ctx: &Context<'_>, key: &str) -> plain_graphql::Result<()> {
    let allowed = if access(key) == Some(PrefAccess::LocalOrFiles) {
        files_preference_allowed(ctx)
    } else {
        let capabilities = preference_capabilities(ctx, key).await?;
        plain_rs::features::prefs::pref_keys::writable(key, is_local(ctx), &capabilities)
    };
    if !allowed {
        return Err(plain_graphql::Error::new("Preference is not accessible"));
    }
    Ok(())
}

async fn preference_capabilities(
    ctx: &Context<'_>,
    key: &str,
) -> plain_graphql::Result<Vec<plain_rs::features::capability::Capability>> {
    if is_local(ctx) || access(key) != Some(PrefAccess::LocalOrHttpServer) {
        return Ok(Vec::new());
    }
    crate::graphql::resolvers::app::http_server::authorized_capabilities(ctx).await
}

fn files_preference_allowed(ctx: &Context<'_>) -> bool {
    use crate::graphql::executor::RequestClient;
    use plain_rs::features::access::AccessFeature;
    is_local(ctx)
        || (ctx.data_opt::<RequestClient>().is_some()
            && crate::http::auth::public_gate::require(ctx, prefs(ctx), &[AccessFeature::Files])
                .is_ok())
}
