use super::events;
use super::models::ClipboardChange;
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};

#[derive(Default)]
pub struct DeviceSubscriptions;
#[Subscription]
impl DeviceSubscriptions {
    async fn device_name_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<String>>> {
        events::stream(ctx, "DEVICE_NAME_UPDATED").await
    }
    async fn access_updated(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "ACCESS_UPDATED").await
    }
    async fn content_changed(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "CONTENT_CHANGED").await
    }
    async fn clipboard_history_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        use crate::graphql::executor::RequestOrigin;
        use crate::server::ServerState;
        use futures_util::{StreamExt, stream};
        let state = ctx.data::<ServerState>()?.clone();
        let origin = *ctx.data::<RequestOrigin>()?;
        let spec = plain_rs::infrastructure::protocols::graphql::events::EVENTS
            .iter()
            .find(|spec| spec.field == "clipboardHistoryChanged")
            .unwrap();
        let changes = state.db.clipboard_changes();
        events::authorize(&state, origin, spec).await?;
        if let Some(ready) = ctx.data_opt::<super::session::SubscriptionReady>() {
            ready.0.send_replace(true);
        }
        Ok(
            stream::unfold((changes, state), move |(mut changes, state)| async move {
                changes.changed().await.ok()?;
                let result = events::authorize(&state, origin, spec).await.map(|_| true);
                Some((result, (changes, state)))
            })
            .boxed(),
        )
    }
    async fn clipboard_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<ClipboardChange>>> {
        events::stream(ctx, "CLIPBOARD_CHANGED").await
    }
}
