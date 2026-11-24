use super::{FEATURE, notification};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::Value;
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::notifications::types::Notification;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct NotificationsQuery;

#[Object]
impl NotificationsQuery {
    async fn notifications(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<Notification>> {
        crate::graphql::resolvers::common::validate_pagination(offset, limit)?;
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::Notifications,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let items = notification_items(ctx, &query).await?;
        Ok(items
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|item| notification(&item))
            .collect())
    }

    async fn notification_count(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<i32> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::Notifications,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(notification_items(ctx, &query).await?.len() as i32)
    }
}

async fn notification_items(ctx: &Context<'_>, query: &str) -> plain_graphql::Result<Vec<Value>> {
    let facts = ctx
        .data_unchecked::<Arc<Host>>()
        .call("systemNotificationFacts", plain_libs::serde_json::json!({}))
        .await
        .map_err(|error| plain_graphql::Error::new(error))?;
    let facts: Vec<Value> = plain_libs::serde_json::from_value(facts)
        .map_err(|error| plain_graphql::Error::from(error))?;
    plain_rs::features::services::notifications::query::notifications(
        ctx.data_unchecked::<Arc<Prefs>>(),
        facts,
        query,
    )
    .map_err(|error| plain_graphql::Error::from(error))
}
