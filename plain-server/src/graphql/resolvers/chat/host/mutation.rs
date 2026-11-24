use super::super::inputs::ChatContentInput;
use super::super::{ChatItem, item};
use super::ChatTextEdit;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::services::chat::chat_actions;

#[derive(Default)]
pub struct ChatHostMutation;

#[Object]
impl ChatHostMutation {
    async fn send_chat_items(
        &self,
        ctx: &Context<'_>,
        targets: Vec<String>,
        content: ChatContentInput,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        let content = content.content()?;
        if super::super::has_channel_target(&targets) {
            super::super::authorize_attachment_import(ctx, &content)?;
        }
        Ok(chat_actions::send_many(
            ctx.data::<ServerState>()?,
            targets,
            plain_libs::serde_json::to_string(&content)?,
        )
        .await?
        .iter()
        .map(item)
        .collect::<plain_graphql::Result<Vec<_>>>()?)
    }

    async fn send_share(
        &self,
        ctx: &Context<'_>,
        targets: Vec<String>,
        share_id: ID,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        Ok(plain_rs::features::services::chat::share_send::existing(
            ctx.data::<ServerState>()?,
            ctx.data::<std::sync::Arc<plain_rs::features::files::shares::Service>>()?
                .clone(),
            targets,
            share_id.to_string(),
        )
        .await?
        .iter()
        .map(item)
        .collect::<plain_graphql::Result<Vec<_>>>()?)
    }

    async fn send_folder_share(
        &self,
        ctx: &Context<'_>,
        targets: Vec<String>,
        path: String,
        name: String,
        expires_at: Option<plain_rs::infrastructure::types::Instant>,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        Ok(plain_rs::features::services::chat::share_send::folder(
            ctx.data::<ServerState>()?,
            ctx.data::<std::sync::Arc<plain_rs::features::files::shares::Service>>()?
                .clone(),
            targets,
            path,
            name,
            expires_at.map(|time| {
                time.0
                    .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
            }),
        )
        .await?
        .iter()
        .map(item)
        .collect::<plain_graphql::Result<Vec<_>>>()?)
    }

    async fn forward_chat_items(
        &self,
        ctx: &Context<'_>,
        id: ID,
        targets: Vec<String>,
        note: Option<String>,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        super::super::authorize_stored_attachment_import(ctx, id.as_str(), Some(&targets))?;
        Ok(chat_actions::forward_many(
            ctx.data::<ServerState>()?,
            id.as_str(),
            targets,
            note.as_deref(),
        )
        .await?
        .iter()
        .map(item)
        .collect::<plain_graphql::Result<Vec<_>>>()?)
    }
    fn edit_chat_text(
        &self,
        ctx: &Context<'_>,
        id: ID,
        text: String,
    ) -> plain_graphql::Result<ChatTextEdit> {
        let state = ctx.data::<ServerState>()?;
        let result = plain_rs::features::services::link_preview::edit(
            &state.db,
            &state.directory,
            id.as_str(),
            &text,
        )?;
        state.previews.request(id.as_str());
        if result.changed {
            chat_actions::committed(
                state,
                &result.chat,
                plain_rs::features::chat::events::WS_MESSAGE_UPDATED,
            );
        }
        Ok(ChatTextEdit {
            changed: result.changed,
            chat: item(&result.chat)?,
        })
    }
}
