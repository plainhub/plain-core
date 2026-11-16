use crate::features::files::resource_link::{self, Resource, Scope};
use plain_libs::serde_json::Value;

pub fn peer_content(
    content: &str,
    client_id: &str,
    resource_token: &str,
) -> plain_libs::anyhow::Result<String> {
    let mut value: Value = plain_libs::serde_json::from_str(content)?;
    match value["type"].as_str() {
        Some("SHARE") => return Ok(content.into()),
        Some("MESSAGE") => {}
        _ => plain_libs::anyhow::bail!("invalid chat content envelope"),
    }
    // Link previews and their cached file references are local to each peer.
    if let Some(message) = value["value"].as_object_mut() {
        message.remove("linkPreviews");
    }
    let items = value
        .get_mut("value")
        .and_then(|v| v.get_mut("items"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid attachment items"))?;
    if items.is_empty() {
        return Ok(value.to_string());
    }
    let key = crate::features::sessions::token_key(resource_token)?;
    let scope = Scope::Session(client_id.to_owned());
    for item in items {
        let item = item
            .get_mut("attachment")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid attachment"))?;
        let uri = item
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing attachment URI"))?;
        let file_id = resource_link::seal(&scope, &key, &Resource::reference(uri))?;
        item.insert("uri".into(), Value::String(format!("fsid:{file_id}")));
    }
    Ok(value.to_string())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/messages/content.rs"]
mod tests;

pub(crate) fn plain_text(content: &Value) -> Option<&str> {
    (content["type"] == "MESSAGE" && content["value"]["format"] == "PLAIN")
        .then(|| content["value"]["text"].as_str())
        .flatten()
}
