pub mod models;
pub mod mutation;

pub mod query;

use crate::http::auth::public_gate;
use plain_graphql::Context;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::clipboard::error::ClipboardErrorCode;
use plain_rs::features::prefs::Prefs;
use std::sync::Arc;

const PERMISSION: AccessFeature = AccessFeature::Clipboard;

fn enabled(ctx: &Context<'_>) -> plain_graphql::Result<()> {
    // plain-app's clipboard roots raise their own error here, not `no_permission`.
    if public_gate::is_local(ctx)
        || plain_rs::features::services::device::access::is_enabled(
            ctx.data_unchecked::<Arc<Prefs>>(),
            &[PERMISSION],
        )
    {
        Ok(())
    } else {
        Err(plain_graphql::Error::from(
            ClipboardErrorCode::ClipboardSyncDisabled,
        ))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/clipboard/mod.rs"]
mod tests;
