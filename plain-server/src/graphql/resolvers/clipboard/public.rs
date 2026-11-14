use plain_rs::features::access::AccessFeature;
use crate::graphql::resolvers::clipboard::models::ClipboardItem;
use crate::graphql::types::ActionResult;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};

use plain_rs::features::prefs::Prefs;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

const PERMISSION: AccessFeature = AccessFeature::Clipboard;
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

#[derive(Default)]
pub struct ClipboardMutation;

#[Object]
impl ClipboardMutation {
    /// Record caller-attributed history only after a successful system write; suppress watcher echoes.
    async fn set_clipboard(&self, ctx: &Context<'_>, text: String) -> plain_graphql::Result<bool> {
        enabled(ctx)?;
        let source = ctx
            .data_opt::<crate::graphql::executor::RequestClient>()
            .map(crate::graphql::executor::RequestClient::client_id)
            .unwrap_or_default();
        plain_rs::features::clipboard::validate_text(&text)
            .map_err(plain_graphql::Error::new)?;
        let _receipt = ctx
            .data_opt::<crate::server::ServerState>()
            .map(|state| state.clipboard_changes.received(&text));
        let result = ctx
            .data::<Arc<Host>>()?
            .call("systemSetClipboard", plain_libs::serde_json::json!({ "text": &text }))
            .await
            .map_err(plain_graphql::Error::new)?;
        let written = plain_libs::serde_json::from_value(result)?;
        if written {
            plain_rs::features::services::clipboard::record_history(
                ctx.data::<Arc<Db>>()?,
                ctx.data::<Arc<Host>>()?,
                &text,
                source,
                false,
            )
            .map_err(plain_graphql::Error::new)?;
        }
        Ok(written)
    }

    /// Delete clipboard history entries matching the query DSL (`ids:` /
    /// `text:`; a bare word is a text LIKE). A blank query is rejected —
    /// send `all:true` to target everything.
    fn delete_clipboard_items(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        enabled(ctx)?;
        if query.trim().is_empty() {
            return Err(plain_graphql::Error::new(
                "query is required for bulk mutations — pass 'all:true' to explicitly target everything",
            ));
        }
        let affected_count = ctx
            .data::<Arc<Db>>()?
            .clipboard_delete_query(&query)
            .map_err(plain_graphql::Error::new)?;
        if affected_count > 0 {
            ctx.data::<Arc<Host>>()?
                .invalidate("clipboardHistoryRefresh");
        }
        Ok(ActionResult {
            affected_count: affected_count.try_into()?,
        })
    }
}

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
        Err(plain_graphql::Error::new("clipboard_sync_disabled"))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/clipboard/public.rs"]
mod tests;
