use super::super::{ChatItem, item};
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
    fn chat_records(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        let state = ctx.data::<ServerState>()?;
        let filter = filter(&query, offset, limit, false);
        let value = plain_rs::infrastructure::db::chat_store::messages::list(&state.db, &filter)?;
        let rows: Vec<plain_rs::infrastructure::db::DChat> =
            plain_libs::serde_json::from_value(value)?;
        plain_rs::features::services::chat::prewarm::prepare_conversation(state, &filter);
        rows.iter().map(item).collect()
    }
    fn chat_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        let state = ctx.data::<ServerState>()?;
        let value = plain_rs::infrastructure::db::chat_store::messages::list(
            &state.db,
            &filter(&query, 0, 0, true),
        )?;
        Ok(i32::try_from(value.as_i64().unwrap_or_default())?)
    }
}

fn filter(
    query: &str,
    offset: i32,
    limit: i32,
    count_only: bool,
) -> plain_rs::infrastructure::db::chat_store::messages::Filter {
    let fields = plain_libs::utils::search_dsl::parse(query);
    let field = |name: &str| {
        fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.value.clone())
    };
    plain_rs::infrastructure::db::chat_store::messages::Filter {
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
