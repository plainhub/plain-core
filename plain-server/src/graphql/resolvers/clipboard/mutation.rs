use super::enabled;
use crate::graphql::types::ActionResult;
use plain_graphql::{Context, Object};
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct ClipboardMutation;

#[Object]
impl ClipboardMutation {
    /// Record caller-attributed history only after a successful system write; suppress watcher echoes.
    async fn set_clipboard(
        &self,
        ctx: &Context<'_>,
        text: String,
        #[graphql(default)] label: String,
        #[graphql(default)] sensitive: bool,
    ) -> plain_graphql::Result<bool> {
        enabled(ctx)?;
        let source = ctx
            .data_opt::<crate::graphql::executor::RequestClient>()
            .map(crate::graphql::executor::RequestClient::client_id)
            .unwrap_or_default();
        plain_rs::features::clipboard::validate_text(&text).map_err(plain_graphql::Error::from)?;
        let _receipt = ctx
            .data_opt::<crate::server::ServerState>()
            .map(|state| state.clipboard_writes.begin(&text));
        let result = ctx
            .data::<Arc<Host>>()?
            .call(
                "systemSetClipboard",
                plain_libs::serde_json::json!({ "text": &text, "label": &label, "sensitive": sensitive }),
            )
            .await
            .map_err(plain_graphql::Error::from)?;
        let written = plain_libs::serde_json::from_value(result)?;
        if written {
            plain_rs::features::services::clipboard::record_write(
                ctx.data::<Arc<Db>>()?,
                ctx.data::<Arc<Host>>()?,
                &text,
                source,
                &label,
                sensitive,
            )
            .map_err(plain_graphql::Error::from)?;
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
            .map_err(plain_graphql::Error::from)?;
        if affected_count > 0 {
            ctx.data::<Arc<Host>>()?
                .invalidate("clipboardHistoryRefresh");
        }
        Ok(ActionResult {
            affected_count: affected_count.try_into()?,
        })
    }
}
