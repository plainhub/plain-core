use super::super::item;
use super::ChatConversation;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object};

#[derive(Default)]
pub struct ChatHostQuery;

#[Object]
impl ChatHostQuery {
    fn chat_conversations(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<Vec<ChatConversation>> {
        Ok(plain_rs::features::chat::cacher::ChatCacher::conversations(
            &ctx.data::<ServerState>()?.db,
        )?
        .into_iter()
        .map(|(id, chat)| {
            Ok(ChatConversation {
                id: ID::from(id),
                chat: item(&chat)?,
            })
        })
        .collect::<plain_graphql::Result<Vec<_>>>()?)
    }
}
