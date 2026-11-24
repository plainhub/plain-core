pub mod bookmark_types;
pub mod metadata;

pub mod mutation;
pub mod query;

use plain_graphql::Context;
use plain_rs::infrastructure::db::bookmark::DBookmark;
use plain_rs::infrastructure::events::WsEvent;
use tokio::sync::broadcast;

pub(crate) fn emit_bookmark_updated(ctx: &Context<'_>, items: &[DBookmark]) {
    plain_rs::features::services::bookmarks::emit_updated(
        ctx.data_unchecked::<broadcast::Sender<WsEvent>>(),
        items,
    );
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/bookmarks/mod.rs"]
mod tests;
