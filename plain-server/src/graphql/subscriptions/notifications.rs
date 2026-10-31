use super::events;
use async_graphql::{Context, Result, Subscription};
use futures_util::stream::BoxStream;
use plain_rs::services::notifications::types::Notification;

#[derive(Default)]
pub struct NotificationsSubscriptions;
#[Subscription]
impl NotificationsSubscriptions {
    async fn notification_created(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Notification>>> {
        events::stream(ctx, "NOTIFICATION_CREATED").await
    }
    async fn notification_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Notification>>> {
        events::stream(ctx, "NOTIFICATION_UPDATED").await
    }
    async fn notification_deleted(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Notification>>> {
        events::stream(ctx, "NOTIFICATION_DELETED").await
    }
    async fn notification_refreshed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "NOTIFICATION_REFRESHED").await
    }
}
