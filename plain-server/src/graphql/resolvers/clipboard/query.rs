use super::enabled;
use crate::graphql::resolvers::clipboard::models::ClipboardItem;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

const MAX_LIMIT: i32 = 200;

#[derive(Default)]
pub struct ClipboardQuery;

#[Object]
impl ClipboardQuery {
    /// Paged clipboard history, newest first.
    fn clipboard_items(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<ClipboardItem>> {
        enabled(ctx)?;
        let (limit, offset) = if public_gate::is_local(ctx) {
            (limit, offset)
        } else {
            (limit.clamp(1, MAX_LIMIT), offset.max(0))
        };
        ctx.data::<Arc<Db>>()?
            .clipboard_page(&query, limit.into(), offset.into())?
            .into_iter()
            .map(ClipboardItem::try_from)
            .collect()
    }

    fn clipboard_item_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        enabled(ctx)?;
        Ok(ctx.data::<Arc<Db>>()?.clipboard_count(&query)?.try_into()?)
    }
}
