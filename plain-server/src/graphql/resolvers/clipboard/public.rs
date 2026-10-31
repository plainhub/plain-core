use crate::graphql::resolvers::clipboard::models::ClipboardItem;
use crate::graphql::types::ActionResult;
use crate::http::auth::public_gate;
use async_graphql::{Context, Object};
use plain_rs::db::Db;
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use std::sync::Arc;

const PERMISSION: &str = "CLIPBOARD";
const MAX_LIMIT: i32 = 200;

#[derive(Default)]
pub struct ClipboardQuery;

#[Object]
impl ClipboardQuery {
    /// Paged clipboard history, newest first.
    async fn clipboard_items(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<ClipboardItem>> {
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

    async fn clipboard_item_count(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<i32> {
        enabled(ctx)?;
        Ok(ctx.data::<Arc<Db>>()?.clipboard_count(&query)?.try_into()?)
    }
}

#[derive(Default)]
pub struct ClipboardMutation;

#[Object]
impl ClipboardMutation {
    /// Write text into the system clipboard on behalf of the calling client.
    /// The write is recorded with the caller as source; the local watcher
    /// sees it but skips re-broadcasting (hash dedup), so there is no loop.
    async fn set_clipboard(&self, ctx: &Context<'_>, text: String) -> async_graphql::Result<bool> {
        enabled(ctx)?;
        let source = ctx
            .data_opt::<crate::graphql::executor::RequestClient>()
            .map(crate::graphql::executor::RequestClient::client_id)
            .unwrap_or_default();
        plain_rs::services::clipboard::record_history(
            ctx.data::<Arc<Db>>()?,
            ctx.data::<Arc<Host>>()?,
            &text,
            source,
            false,
        )
        .map_err(async_graphql::Error::new)?;
        if let Some(state) = ctx.data_opt::<crate::server::ServerState>() {
            state.clipboard_changes.received(&text);
        }
        let result = ctx
            .data::<Arc<Host>>()?
            .call("systemSetClipboard", serde_json::json!({ "text": text }))
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(serde_json::from_value(result)?)
    }

    /// Delete clipboard history entries matching the query DSL (`ids:` /
    /// `text:`; a bare word is a text LIKE). A blank query is rejected —
    /// send `all:true` to target everything.
    async fn delete_clipboard_items(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<ActionResult> {
        enabled(ctx)?;
        if query.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "query is required for bulk mutations — pass 'all:true' to explicitly target everything",
            ));
        }
        let affected_count = ctx
            .data::<Arc<Db>>()?
            .clipboard_delete_query(&query)
            .map_err(async_graphql::Error::new)?;
        if affected_count > 0 {
            ctx.data::<Arc<Host>>()?
                .invalidate("clipboardHistoryRefresh");
        }
        Ok(ActionResult {
            affected_count: affected_count.try_into()?,
        })
    }
}

fn enabled(ctx: &Context<'_>) -> async_graphql::Result<()> {
    // plain-app's clipboard roots raise their own error here, not `no_permission`.
    if public_gate::is_local(ctx)
        || plain_rs::services::device::access::is_enabled(
            ctx.data_unchecked::<Arc<Prefs>>(),
            &[PERMISSION],
        )
    {
        Ok(())
    } else {
        Err(async_graphql::Error::new("clipboard_sync_disabled"))
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/clipboard/public.rs"]
mod tests;
