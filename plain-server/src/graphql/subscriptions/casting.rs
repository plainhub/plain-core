use super::events;
use crate::graphql::resolvers::casting::sender::DlnaSenderState;
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};

#[derive(Default)]
pub struct CastingSubscriptions;

#[Subscription]
impl CastingSubscriptions {
    async fn dlna_sender_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<DlnaSenderState>>> {
        events::stream(ctx, "DLNA_SENDER_CHANGED").await
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/graphql/subscriptions/casting.rs"]
mod tests;
