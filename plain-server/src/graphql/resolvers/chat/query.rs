use super::{ChatChannel, ChatItem, actor, channel, db, item, rows};
use plain_graphql::{Context, Enum, Object};
use plain_rs::infrastructure::db::chat_store::messages::{self, Filter};

#[derive(Clone, Copy, Enum, Eq, PartialEq)]
enum ChatSortBy {
    NewestFirst,
    OldestFirst,
}

#[derive(Default)]
pub struct ChatQuery;

#[Object]
impl ChatQuery {
    fn chat_items(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: ChatSortBy,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        let filter = chat_filter(&query)?;
        let rows = messages::list(
            db(ctx)?,
            &filter,
            offset.into(),
            Some(limit.into()),
            sort_by == ChatSortBy::NewestFirst,
        )?;
        if let Some(state) = ctx.data_opt::<crate::server::ServerState>() {
            plain_rs::features::services::chat::prewarm::prepare_conversation(
                state, &filter, offset, limit,
            );
        }
        rows.iter().map(item).collect()
    }

    fn chat_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        Ok(i32::try_from(messages::count(
            db(ctx)?,
            &chat_filter(&query)?,
        )?)?)
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
                if let Err(error) =
                    plain_rs::features::services::chat::channel_runtime::refresh_from_owners(&state)
                        .await
                {
                    log::debug!("Refresh channels: {error}");
                }
            });
        }
        Ok(rows(&facts, channel))
    }
}

fn chat_filter(query: &str) -> plain_graphql::Result<Filter> {
    let (mut peer, mut channel, mut text) = (None, None, None);
    for field in plain_libs::utils::search_dsl::parse(query) {
        let slot = match field.name.as_str() {
            "peer" if !field.value.trim().is_empty() => &mut peer,
            "channel" if !field.value.trim().is_empty() => &mut channel,
            "text" => &mut text,
            _ => {
                return Err(plain_graphql::Error::new(format!(
                    "Invalid chat filter: {}",
                    field.name
                )));
            }
        };
        if slot.replace(field.value).is_some() {
            return Err(plain_graphql::Error::new(format!(
                "Duplicate chat filter: {}",
                field.name
            )));
        }
    }
    Ok(Filter {
        peer,
        channel,
        text: text.unwrap_or_default(),
    })
}
