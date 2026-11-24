use super::FEATURE;
use crate::graphql::types::ActionResult;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::Value;
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::notifications::error::NotificationErrorCode;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct NotificationsMutation;

#[Object]
impl NotificationsMutation {
    async fn delete_notifications(
        &self,
        ctx: &Context<'_>,
        ids: Vec<plain_graphql::ID>,
    ) -> plain_graphql::Result<ActionResult> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::Notifications,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let ids: Vec<String> = ids.iter().map(|id| id.0.clone()).collect();
        let receipt = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemCancelNotifications",
                plain_libs::serde_json::json!({ "ids": ids }),
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        let canceled: Vec<String> = plain_libs::serde_json::from_value(receipt)
            .map_err(|error| plain_graphql::Error::from(error))?;
        // The platform only confirms what it actually dismissed; a receipt
        // naming an id that was never sent is a host contract violation.
        if canceled.iter().any(|id| !ids.contains(id)) {
            return Err(plain_graphql::Error::new(
                "invalid notification cancellation receipt",
            ));
        }
        if !canceled.is_empty() {
            let _ = ctx
                .data_unchecked::<tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>>()
                .send(plain_rs::infrastructure::events::WsEvent::broadcast(
                    "NOTIFICATION_REFRESHED",
                    "{}".into(),
                ));
        }
        Ok(ActionResult {
            affected_count: canceled.len() as i32,
        })
    }

    /// `actionIndex` indexes [`Notification::reply_actions`], the
    /// reply-capable subset — not the plain action list.
    async fn reply_notification(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        action_index: i32,
        text: String,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        if action_index < 0 {
            return Err(plain_graphql::Error::from(
                NotificationErrorCode::ActionNotFound,
            ));
        }
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::Notifications,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let receipt = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemReplyNotification",
                plain_libs::serde_json::json!({
                    "id": id.0,
                    "actionIndex": action_index,
                    "text": text,
                }),
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        if receipt != Value::Bool(true) {
            return Err(plain_graphql::Error::from(
                NotificationErrorCode::ActionNotFound,
            ));
        }
        Ok(true)
    }
}
