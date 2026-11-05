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
    async fn clipboard_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<ClipboardChange>>> {
        events::stream(ctx, "CLIPBOARD_CHANGED").await
    }
}
