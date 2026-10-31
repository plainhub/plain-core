use super::public::{ChatItem, item};
use crate::server::ServerState;
use async_graphql::{Context, ID, Object, SimpleObject};
use plain_rs::services::chat::chat_actions;

#[derive(SimpleObject)]
pub struct ChatConversation {
    pub id: ID,
    pub chat: ChatItem,
}
#[derive(Default)]
pub struct ChatHostQuery;
#[Object]
impl ChatHostQuery {
    async fn chat_conversations(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<ChatConversation>> {
        Ok(
            plain_rs::chat::cacher::ChatCacher::conversations(&ctx.data::<ServerState>()?.db)?
                .into_iter()
                .map(|(id, chat)| ChatConversation {
                    id: ID::from(id),
                    chat: item(&chat),
                })
                .collect(),
        )
    }
    async fn chat_records(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        let state = ctx.data::<ServerState>()?;
        let filter = filter(&query, offset, limit, false);
        let value = plain_rs::db::chat_store::messages::list(&state.db, &filter)?;
        let rows: Vec<plain_rs::db::DChat> = serde_json::from_value(value)?;
        plain_rs::services::chat::prewarm::prepare_conversation(state, &filter);
        Ok(rows.iter().map(item).collect())
    }
    async fn chat_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        let state = ctx.data::<ServerState>()?;
        let value =
            plain_rs::db::chat_store::messages::list(&state.db, &filter(&query, 0, 0, true))?;
        Ok(i32::try_from(value.as_i64().unwrap_or_default())?)
    }
}
#[derive(SimpleObject)]
pub struct ChatTextEdit {
    pub changed: bool,
    pub chat: ChatItem,
}
#[derive(Default)]
pub struct ChatHostMutation;
#[Object]
impl ChatHostMutation {
    async fn send_chat_items(
        &self,
        ctx: &Context<'_>,
        targets: Vec<String>,
        content: String,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        Ok(
            chat_actions::send_many(ctx.data::<ServerState>()?, targets, content)
                .await?
                .iter()
                .map(item)
                .collect(),
        )
    }

    async fn send_share(
        &self,
        ctx: &Context<'_>,
        targets: Vec<String>,
        share_id: ID,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        Ok(plain_rs::services::chat::share_send::existing(
            ctx.data::<ServerState>()?,
            ctx.data::<std::sync::Arc<plain_rs::files::shares::Service>>()?
                .clone(),
            targets,
            share_id.to_string(),
        )
        .await?
        .iter()
        .map(item)
        .collect())
    }

    async fn send_folder_share(
        &self,
        ctx: &Context<'_>,
        targets: Vec<String>,
        path: String,
        name: String,
        expires_at: Option<plain_rs::types::Instant>,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        Ok(plain_rs::services::chat::share_send::folder(
            ctx.data::<ServerState>()?,
            ctx.data::<std::sync::Arc<plain_rs::files::shares::Service>>()?
                .clone(),
            targets,
            path,
            name,
            expires_at.map(|time| time.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        )
        .await?
        .iter()
        .map(item)
        .collect())
    }

    async fn forward_chat_items(
        &self,
        ctx: &Context<'_>,
        id: ID,
        targets: Vec<String>,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        Ok(
            chat_actions::forward_many(ctx.data::<ServerState>()?, id.as_str(), targets)
                .await?
                .iter()
                .map(item)
                .collect(),
        )
    }
    async fn deliver_chat_item(
        &self,
        ctx: &Context<'_>,
        id: ID,
        recipients: Option<Vec<ID>>,
    ) -> async_graphql::Result<ChatItem> {
        let recipients = recipients.map(|ids| ids.into_iter().map(|id| id.to_string()).collect());
        let receipt = plain_rs::services::chat::chat_delivery::deliver(
            ctx.data::<ServerState>()?,
            id.as_str(),
            recipients,
        )
        .await?;
        Ok(item(&receipt.chat.ok_or_else(|| {
            async_graphql::Error::new("Message unavailable")
        })?))
    }
    async fn edit_chat_text(
        &self,
        ctx: &Context<'_>,
        id: ID,
        text: String,
    ) -> async_graphql::Result<ChatTextEdit> {
        let state = ctx.data::<ServerState>()?;
        let result = plain_rs::services::link_preview::edit(
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
                plain_rs::chat::events::WS_MESSAGE_UPDATED,
            );
        }
        Ok(ChatTextEdit {
            changed: result.changed,
            chat: item(&result.chat),
        })
    }
}

fn filter(
    query: &str,
    offset: i32,
    limit: i32,
    count_only: bool,
) -> plain_rs::db::chat_store::messages::Filter {
    let fields = plain_rs::utils::search_dsl::parse(query);
    let field = |name: &str| {
        fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.value.clone())
    };
    plain_rs::db::chat_store::messages::Filter {
        peer: field("peer"),
        channel: field("channel"),
        text: field("text").unwrap_or_default(),
        offset: i64::from(offset),
        limit: Some(i64::from(limit)),
        descending: field("sort").as_deref() == Some("desc"),
        latest: field("latest").as_deref() == Some("true"),
        count_only,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/chat/host.rs"]
mod tests;
