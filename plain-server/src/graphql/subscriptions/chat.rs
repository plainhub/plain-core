use super::events;
use crate::graphql::resolvers::chat::public::ChatItem;
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};

#[derive(Default)]
pub struct ChatSubscriptions;
#[Subscription]
impl ChatSubscriptions {
    async fn messages_created(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Vec<ChatItem>>>> {
        events::stream(ctx, "MESSAGE_CREATED").await
    }
    async fn messages_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Vec<ChatItem>>>> {
        events::stream(ctx, "MESSAGE_UPDATED").await
    }
    async fn messages_deleted(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<String>>> {
        events::stream(ctx, "MESSAGE_DELETED").await
    }
}
