use super::events;
use crate::graphql::resolvers::casting::sender::DlnaSenderState;
use async_graphql::{Context, Result, Subscription};
use futures_util::stream::BoxStream;

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
#[path = "../../../tests/unit/graphql/subscriptions/casting.rs"]
mod tests;
