use super::content::*;
use plain_graphql::{ID, InputObject};
use plain_rs::infrastructure::types::{Instant, Long};

#[derive(InputObject)]
pub struct ChatLinkPreviewInput {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub image_local_path: Option<String>,
    #[graphql(default)]
    pub image_width: i32,
    #[graphql(default)]
    pub image_height: i32,
    pub site_name: Option<String>,
    pub domain: Option<String>,
    pub created_at: Instant,
}
impl From<ChatLinkPreviewInput> for ChatLinkPreview {
    fn from(value: ChatLinkPreviewInput) -> Self {
        Self {
            url: value.url,
            title: value.title,
            description: value.description,
            image_url: value.image_url,
            image_local_path: value.image_local_path,
            image_width: value.image_width,
            image_height: value.image_height,
            site_name: value.site_name,
            domain: value.domain,
            created_at: value.created_at,
        }
    }
}

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
pub struct ChatTextInput {
    pub text: String,
    #[graphql(default)]
    pub link_previews: Vec<ChatLinkPreviewInput>,
}
impl From<ChatTextInput> for ChatTextContent {
    fn from(value: ChatTextInput) -> Self {
        Self {
            text: value.text,
            link_previews: value.link_previews.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(InputObject)]
pub struct ChatImagesInput {
    pub items: Vec<ChatAttachmentInput>,
}
impl From<ChatImagesInput> for ChatImagesContent {
    fn from(value: ChatImagesInput) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(InputObject)]
pub struct ChatFilesInput {
    pub items: Vec<ChatAttachmentInput>,
}
impl From<ChatFilesInput> for ChatFilesContent {
    fn from(value: ChatFilesInput) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
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
    pub text: Option<ChatTextInput>,
    pub images: Option<ChatImagesInput>,
    pub files: Option<ChatFilesInput>,
    pub share: Option<ChatShareInput>,
}
impl ChatContentInput {
    pub fn content(self) -> plain_graphql::Result<ChatContent> {
        let attachments = self
            .images
            .as_ref()
            .map(|value| &value.items)
            .or_else(|| self.files.as_ref().map(|value| &value.items));
        if attachments.is_some_and(|items| items.iter().any(|item| item.id.is_empty())) {
            return Err("Chat attachment id must be a non-empty string".into());
        }
        match (self.text, self.images, self.files, self.share) {
            (Some(value), None, None, None) => Ok(ChatContent::Text(value.into())),
            (None, Some(value), None, None) => Ok(ChatContent::Images(value.into())),
            (None, None, Some(value), None) => Ok(ChatContent::Files(value.into())),
            (None, None, None, Some(value)) => Ok(ChatContent::Share(value.into())),
            _ => Err("Exactly one chat content field required".into()),
        }
    }
}
