use crate::infrastructure::protocols::graphql::events as registry;
use crate::infrastructure::protocols::graphql::events::EventSpec;
use crate::infrastructure::protocols::graphql::messages::ClientMessage;
use crate::infrastructure::protocols::graphql::messages::ServerMessage;
use crate::infrastructure::protocols::websocket;
use plain_libs::anyhow::{Result, anyhow, ensure};

pub(super) fn operations() -> impl Iterator<Item = &'static EventSpec> {
    registry::EVENTS.iter().filter(|spec| {
        matches!(
            spec.event,
            "NOTIFICATION_CREATED"
                | "NOTIFICATION_UPDATED"
                | "NOTIFICATION_DELETED"
                | "NOTIFICATION_REFRESHED"
                | "ACCESS_UPDATED"
                | "DEVICE_NAME_UPDATED"
                | "CLIPBOARD_CHANGED"
        )
    })
}
pub(super) fn encode(
    message: &ClientMessage,
    key: &[u8],
    stream: &mut websocket::Stream,
) -> Result<Vec<u8>> {
    stream
        .encode(&plain_libs::serde_json::to_vec(message)?, key)
        .ok_or_else(|| anyhow!("Management sequence exhausted"))
}
pub(super) enum Incoming {
    Event(&'static str),
    Pong,
    Ready,
    Rejected(String),
}
pub(super) fn event(frame: &[u8], key: &[u8], stream: &mut websocket::Stream) -> Result<Incoming> {
    let plain = stream
        .decode(frame, key)
        .ok_or_else(|| anyhow!("Invalid management event authentication"))?;
    match plain_libs::serde_json::from_slice::<ServerMessage>(&plain)? {
        ServerMessage::Pong { .. } => Ok(Incoming::Pong),
        ServerMessage::Next { id, payload } => {
            let spec = operations()
                .find(|spec| spec.field == id)
                .ok_or_else(|| anyhow!("Unknown management operation"))?;
            ensure!(
                payload.errors.is_empty(),
                "Management subscription result: {:?}",
                payload.errors
            );
            if payload.extensions.get("subscriptionReady")
                == Some(&plain_graphql::Value::Boolean(true))
            {
                return Ok(Incoming::Ready);
            }
            let mut data = plain_libs::serde_json::to_value(payload.data)?;
            let _value = data
                .as_object_mut()
                .and_then(|data| data.remove(spec.field))
                .ok_or_else(|| anyhow!("Missing management subscription data"))?;
            Ok(Incoming::Event(spec.event))
        }
        ServerMessage::Error { id, payload } => Ok(Incoming::Rejected(format!(
            "{id}: {}",
            payload
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
        ServerMessage::Complete { id } => Err(anyhow!("Management subscription completed: {id}")),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/management_wire.rs"]
mod tests;
