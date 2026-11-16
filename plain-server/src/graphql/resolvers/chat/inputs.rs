use super::content::*;
use plain_graphql::{ID, InputObject};
use plain_rs::infrastructure::types::{Instant, Long};

#[derive(InputObject)]
pub struct ChatAttachmentInput {
    pub id: ID,
    pub uri: String,
    pub size: Long,
    #[graphql(default)]
    pub duration_ms: Long,
    #[graphql(default)]
    pub width: i32,
    #[graphql(default)]
    pub height: i32,
    #[graphql(default)]
    pub summary: String,
    #[graphql(default)]
    pub file_name: String,
}
impl From<ChatAttachmentInput> for ChatAttachment {
    fn from(value: ChatAttachmentInput) -> Self {
        Self {
            id: value.id.into(),
            uri: value.uri,
            size: value.size,
            duration_ms: value.duration_ms,
            width: value.width,
            height: value.height,
            summary: value.summary,
            file_name: value.file_name,
        }
    }
}

#[derive(InputObject)]
pub struct ChatSharePeerInput {
    pub id: ID,
    pub ip: String,
    pub port: i32,
}
impl From<ChatSharePeerInput> for ChatSharePeer {
    fn from(value: ChatSharePeerInput) -> Self {
        Self {
            id: value.id,
            ip: value.ip,
            port: value.port,
        }
    }
}

#[derive(InputObject)]
pub struct ChatMessageAttachmentInput {
    #[graphql(name = "type")]
    pub kind: ChatAttachmentType,
    pub attachment: ChatAttachmentInput,
}

#[derive(InputObject)]
pub struct ChatMessageInput {
    pub text: Option<String>,
    #[graphql(default)]
    pub format: ChatTextFormat,
    #[graphql(default)]
    pub items: Vec<ChatMessageAttachmentInput>,
}

impl From<ChatMessageInput> for ChatMessageContent {
    fn from(value: ChatMessageInput) -> Self {
        Self {
            text: value.text,
            format: value.format,
            items: value
                .items
                .into_iter()
                .map(|item| ChatMessageAttachment {
                    kind: item.kind,
                    attachment: item.attachment.into(),
                })
                .collect(),
            link_previews: Vec::new(),
        }
    }
}

#[derive(InputObject)]
pub struct ChatShareInput {
    pub share_id: ID,
    pub url_token: String,
    pub peer_info: ChatSharePeerInput,
    pub name: String,
    #[graphql(default)]
    pub item_count: Long,
    #[graphql(default)]
    pub total_size: Long,
    pub expires_at: Option<Instant>,
}
impl From<ChatShareInput> for ChatShareContent {
    fn from(value: ChatShareInput) -> Self {
        Self {
            share_id: value.share_id,
            url_token: value.url_token,
            peer_info: value.peer_info.into(),
            name: value.name,
            item_count: value.item_count,
            total_size: value.total_size,
            expires_at: value.expires_at,
        }
    }
}

#[derive(InputObject)]
#[graphql(oneof)]
pub struct ChatContentInput {
    pub message: Option<ChatMessageInput>,
    pub share: Option<ChatShareInput>,
}

impl ChatContentInput {
    pub fn content(self) -> plain_graphql::Result<ChatContent> {
        match (self.message, self.share) {
            (Some(value), None) => Ok(ChatContent::Message(value.into())),
            (None, Some(value)) => Ok(ChatContent::Share(value.into())),
            _ => Err("Exactly one chat content field required".into()),
        }
    }
}
