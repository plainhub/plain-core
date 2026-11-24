pub mod mutation;
pub mod query;

use crate::graphql::types::facts::{flag, id, instant, strings, text};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::services::notifications::types::Notification;

const FEATURE: AccessFeature = AccessFeature::Notifications;

pub(crate) fn notification(value: &Value) -> Notification {
    Notification {
        id: id(value, "id"),
        only_once: flag(value, "onlyOnce"),
        is_clearable: flag(value, "isClearable"),
        app_id: id(value, "appId"),
        app_name: text(value, "appName"),
        posted_at: instant(value, "postedAt"),
        silent: flag(value, "silent"),
        title: text(value, "title"),
        body: text(value, "body"),
        actions: strings(value, "actions"),
        reply_actions: strings(value, "replyActions"),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/notifications/mod.rs"]
mod tests;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventFacts {
    #[serde(rename = "type")]
    event_type: String,
    notification: Value,
}

pub(crate) fn publish(
    prefs: &plain_rs::features::prefs::Prefs,
    events: &tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>,
    facts: EventFacts,
) -> Result<(), String> {
    let kind = match facts.event_type.as_str() {
        "NOTIFICATION_CREATED" => "NOTIFICATION_CREATED",
        "NOTIFICATION_UPDATED" => "NOTIFICATION_UPDATED",
        "NOTIFICATION_DELETED" => "NOTIFICATION_DELETED",
        _ => return Err("invalid notification event type".into()),
    };
    if kind != "NOTIFICATION_DELETED"
        && (!plain_rs::features::services::device::access::is_enabled(prefs, &[FEATURE])
            || plain_rs::features::services::notifications::query::notifications(
                prefs,
                vec![facts.notification.clone()],
                "",
            )
            .map_err(|error| error.to_string())?
            .is_empty())
    {
        return Ok(());
    }
    let payload = plain_libs::serde_json::to_string(&notification(&facts.notification))
        .map_err(|error| error.to_string())?;
    let _ = events.send(plain_rs::infrastructure::events::WsEvent::broadcast(
        kind, payload,
    ));
    if kind != "NOTIFICATION_DELETED" {
        plain_rs::features::services::sms::provider_events::notification_changed(
            events,
            facts.notification["appId"].as_str().unwrap_or_default(),
        );
    }
    Ok(())
}
