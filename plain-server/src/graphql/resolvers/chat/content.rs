use plain_graphql::{Enum, ID, SimpleObject, Union};
use plain_rs::infrastructure::types::{Instant, Long};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ChatLinkPreview {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    #[serde(default)]
    pub image_width: i32,
    #[serde(default)]
    pub image_height: i32,
    pub site_name: Option<String>,
    pub domain: Option<String>,
    pub created_at: Instant,
}

#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    #[serde(default)]
    pub id: ID,
    pub uri: String,
    pub size: Long,
    #[serde(default)]
    pub duration_ms: Long,
    #[serde(default)]
    pub width: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub file_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ChatSharePeer {
    pub id: ID,
    pub ip: String,
    pub port: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChatTextFormat {
    #[default]
    Plain,
    Markdown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, Enum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChatAttachmentType {
    Image,
    File,
}

#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject)]
pub struct ChatMessageAttachment {
    #[serde(rename = "type")]
    #[graphql(name = "type")]
    pub kind: ChatAttachmentType,
    pub attachment: ChatAttachment,
}

#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageContent {
    pub text: Option<String>,
    pub format: ChatTextFormat,
    pub items: Vec<ChatMessageAttachment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_previews: Vec<ChatLinkPreview>,
}

#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ChatShareContent {
    pub share_id: ID,
    pub url_token: String,
    pub peer_info: ChatSharePeer,
    pub name: String,
    #[serde(default)]
    pub item_count: Long,
    #[serde(default)]
    pub total_size: Long,
    pub expires_at: Option<Instant>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Union)]
#[serde(tag = "type", content = "value")]
pub enum ChatContent {
    #[serde(rename = "MESSAGE")]
    Message(ChatMessageContent),
    #[serde(rename = "SHARE")]
    Share(ChatShareContent),
}

#[derive(Clone, Debug, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct ChatDeliveryResult {
    pub peer_id: ID,
    pub peer_name: String,
    pub error: Option<String>,
}

pub fn read_content<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ChatContent, D::Error> {
    let value = String::deserialize(deserializer)?;
    plain_libs::serde_json::from_str(&value).map_err(<D::Error as serde::de::Error>::custom)
}

pub fn read_deliveries<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<ChatDeliveryResult>, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(Deserialize)]
    struct Results {
        results: Option<Vec<ChatDeliveryResult>>,
    }
    plain_libs::serde_json::from_str::<Results>(&value)
        .map(|value| value.results.unwrap_or_default())
        .map_err(<D::Error as serde::de::Error>::custom)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/chat/content.rs"]
mod tests;
