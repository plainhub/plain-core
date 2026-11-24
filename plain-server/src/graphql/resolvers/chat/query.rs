use super::{ChatChannel, ChatItem, actor, channel, db, item, rows};
use plain_graphql::{Context, Object};

#[derive(Default)]
pub struct ChatQuery;

#[Object]
impl ChatQuery {
    /// Messages in chronological order; `target`: `peer:<id>` or `channel:<id>`.
    fn chat_items(
        &self,
        ctx: &Context<'_>,
        target: String,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        let text = text_of(&query);
        let rows = db(ctx)?.get_chats_page(&target, &text, offset, limit)?;
        if let Some(state) = ctx.data_opt::<crate::server::ServerState>() {
            plain_rs::features::services::chat::prewarm::prepare_page(
                state, &target, &text, offset, limit,
            );
        }
        rows.iter().map(item).collect()
    }

    fn chat_item(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<Option<ChatItem>> {
        Ok(
            plain_rs::infrastructure::db::chat_store::messages::get(db(ctx)?, id.as_str())?
                .as_ref()
                .map(item)
                .transpose()?,
        )
    }

    fn chat_channels(
        &self,
        ctx: &Context<'_>,
        #[graphql(default)] refresh_from_owners: bool,
    ) -> plain_graphql::Result<Vec<ChatChannel>> {
        let facts = plain_rs::features::chat::events::public_channels(db(ctx)?, &actor(ctx)?)?;
        if refresh_from_owners {
            let state = ctx.data::<crate::server::ServerState>()?.clone();
            tokio::spawn(async move {
                if let Err(error) = plain_rs::features::services::chat::channel_runtime::refresh_from_owners(&state).await {
                    log::debug!("Refresh channels: {error}");
                }
            });
        }
        Ok(rows(&facts, channel))
    }
}

fn text_of(query: &str) -> String {
    plain_libs::utils::search_dsl::parse(query)
        .into_iter()
        .find(|field| field.name == "text")
        .map(|field| field.value)
        .unwrap_or_default()
}
