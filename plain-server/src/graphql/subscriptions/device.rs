use super::events;
use super::models::ClipboardChange;
use async_graphql::{Context, Result, Subscription};
use futures_util::stream::BoxStream;

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
    async fn permissions_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "PERMISSIONS_UPDATED").await
    }
    async fn content_changed(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "CONTENT_CHANGED").await
    }
    async fn clipboard_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<ClipboardChange>>> {
        events::stream(ctx, "CLIPBOARD_CHANGED").await
    }
}
