use crate::features::files::resource_link::{self, Resource, Scope};
use crate::infrastructure::db::Db;
use serde_json::Value;

pub fn peer_content(content: &str, client_id: &str, db: &Db) -> plain_libs::anyhow::Result<String> {
    let mut value: Value = serde_json::from_str(content)?;
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
