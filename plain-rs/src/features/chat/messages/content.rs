use crate::features::files::resource_link::{self, Resource, Scope};
use crate::infrastructure::db::Db;
use plain_libs::serde_json::Value;

pub fn peer_content(content: &str, client_id: &str, db: &Db) -> plain_libs::anyhow::Result<String> {
    let mut value: Value = plain_libs::serde_json::from_str(content)?;
    if !matches!(
        value.get("type").and_then(Value::as_str),
        Some("FILES" | "IMAGES")
    ) {
        return Ok(content.into());
    }
    let session = db
        .session_get(client_id)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Session unavailable"))?;
    let key = crate::features::sessions::token_key(&session.url_token)?;
    let scope = Scope::Session(session.client_id);
    let items = value
        .get_mut("value")
        .and_then(|v| v.get_mut("items"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid attachment items"))?;
    for item in items {
        let uri = item
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing attachment URI"))?;
        let file_id = resource_link::seal(&scope, &key, &Resource::reference(uri))?;
        item.as_object_mut()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid attachment"))?
            .insert("uri".into(), Value::String(format!("fsid:{file_id}")));
    }
    Ok(value.to_string())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/messages/content.rs"]
mod tests;

pub fn attachment_duration_ms(item: &Value) -> plain_libs::anyhow::Result<i64> {
    let read = |name: &str| -> plain_libs::anyhow::Result<Option<i64>> {
        match item.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value
                .as_i64()
                .filter(|value| *value >= 0)
                .map(Some)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid attachment duration")),
        }
    };
    let milliseconds = read("durationMs")?;
    let legacy_seconds = read("legacyDurationSec")?;
    let seconds = read("duration")?;
    if let Some(value) = milliseconds {
        return Ok(value);
    }
    let scale = |value: Option<i64>| -> plain_libs::anyhow::Result<Option<i64>> {
        value
            .map(|value| {
                value
                    .checked_mul(1000)
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("invalid attachment duration"))
            })
            .transpose()
    };
    let legacy = scale(legacy_seconds)?;
    let duration = scale(seconds)?;
    Ok(legacy.or(duration).unwrap_or_default())
}
