use anyhow::{Result, anyhow, ensure};
use plain_rs::protocols::graphql::events as registry;
use plain_rs::protocols::graphql::events::EventSpec;
use plain_rs::protocols::graphql::messages::ClientMessage;
use plain_rs::protocols::graphql::messages::ServerMessage;
use plain_rs::protocols::graphql::{self, Role};

pub(super) fn operations() -> impl Iterator<Item = &'static EventSpec> {
    registry::EVENTS.iter().filter(|spec| {
        matches!(
            spec.event,
            "NOTIFICATION_CREATED"
                | "NOTIFICATION_UPDATED"
                | "NOTIFICATION_DELETED"
                | "NOTIFICATION_REFRESHED"
                | "PERMISSIONS_UPDATED"
                | "DEVICE_NAME_UPDATED"
                | "CLIPBOARD_CHANGED"
        )
    })
}
pub(super) fn registration(key: &[u8], client: &str, timestamp: i64) -> Result<Vec<u8>> {
    let init = plain_rs::protocols::graphql::Init {
        timestamp_millis: timestamp,
        client_nonce: plain_rs::utils::hex::bytes_to_hex(&plain_rs::crypto::random_bytes(32)),
    };
    graphql::seal_init(key, client, &serde_json::to_vec(&init)?)
        .ok_or_else(|| anyhow!("Invalid management session key"))
}
pub(super) fn acknowledge(
    key: &[u8],
    client: &str,
    init: &[u8],
    ack: &[u8],
) -> Result<graphql::Stream> {
    let plain = graphql::open_ack(key, client, init, ack)
        .ok_or_else(|| anyhow!("Invalid management acknowledgement"))?;
    let ack_body: plain_rs::protocols::graphql::Ack = serde_json::from_slice(&plain)?;
    ensure!(
        ack_body.server_nonce.len() == 64
            && ack_body
                .server_nonce
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "Invalid management nonce"
    );
    graphql::Stream::new(client, init, ack, Role::Client)
        .ok_or_else(|| anyhow!("Invalid management binding"))
}
pub(super) fn encode(
    message: &ClientMessage,
    key: &[u8],
    stream: &mut graphql::Stream,
) -> Result<Vec<u8>> {
    stream
        .encode(&serde_json::to_vec(message)?, key)
        .ok_or_else(|| anyhow!("Management sequence exhausted"))
}
pub(super) enum Incoming {
    Event(&'static str),
    Pong,
    Ready,
    Rejected(String),
}
pub(super) fn event(frame: &[u8], key: &[u8], stream: &mut graphql::Stream) -> Result<Incoming> {
    let plain = stream
        .decode(frame, key)
        .ok_or_else(|| anyhow!("Invalid management event authentication"))?;
    match serde_json::from_slice::<ServerMessage>(&plain)? {
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
                == Some(&async_graphql::Value::Boolean(true))
            {
                return Ok(Incoming::Ready);
            }
            let mut data = serde_json::to_value(payload.data)?;
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
#[path = "../../../tests/unit/services/peers/management_wire.rs"]
mod tests;
