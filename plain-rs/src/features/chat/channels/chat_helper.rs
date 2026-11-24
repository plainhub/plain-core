use crate::features::chat::enums::ChatStatus;
use plain_libs::serde_json::{Value, json};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelDeliveryResult {
    pub peer_id: String,
    pub peer_name: String,
    pub error: Option<String>,
}

impl ChannelDeliveryResult {
    pub fn to_json(&self) -> Value {
        json!({
            "peerId": self.peer_id,
            "peerName": self.peer_name,
            "error": self.error.as_deref().map_or(Value::Null, |e| json!(e)),
        })
    }
}

pub fn compute_status(results: &[ChannelDeliveryResult]) -> ChatStatus {
    if results.is_empty() {
        return ChatStatus::Sent;
    }
    let failed = results.iter().filter(|r| r.error.is_some()).count();
    if failed == 0 {
        ChatStatus::Sent
    } else if failed == results.len() {
        ChatStatus::Failed
    } else {
        ChatStatus::Partial
    }
}

pub fn build_status_data_json(results: &[ChannelDeliveryResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let arr: Vec<Value> = results.iter().map(|r| r.to_json()).collect();
    json!({ "results": arr }).to_string()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/channels/chat_helper.rs"]
mod tests;
