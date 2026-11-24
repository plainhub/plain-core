use super::{ChatChannel, ChatItem, action, item};
use crate::graphql::types::ActionResult;
use plain_graphql::{Context, Object};
use plain_rs::features::services::chat::channel_runtime::Request as ChannelRequest;

#[derive(Default)]
pub struct ChatMutation;

#[Object]
impl ChatMutation {
    /// Always true: an id that is already gone is the state the caller asked
    /// for, and the contract has no way to say "nothing was there".
    fn delete_chat_item(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        plain_rs::features::services::chat::chat_actions::delete(
            ctx.data::<crate::server::ServerState>()?,
            vec![id.to_string()],
        )?;
        Ok(true)
    }

    fn delete_chat_items(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        let count = plain_rs::features::services::chat::chat_actions::delete_query(
            ctx.data::<crate::server::ServerState>()?,
            &query,
        )?;
        Ok(ActionResult {
            affected_count: i32::try_from(count)?,
        })
    }

    async fn retry_chat_item(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        recipient_ids: Option<Vec<plain_graphql::ID>>,
    ) -> plain_graphql::Result<ChatItem> {
        let row = plain_rs::features::services::chat::chat_actions::retry(
            ctx.data::<crate::server::ServerState>()?,
            id.to_string(),
            recipient_ids.map(|ids| ids.into_iter().map(|id| id.to_string()).collect()),
        )
        .await?;
        item(&row)
    }

    async fn create_chat_channel(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> plain_graphql::Result<ChatChannel> {
        action(ctx, ChannelRequest::Create { name }).await
    }

    async fn update_chat_channel(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        name: String,
    ) -> plain_graphql::Result<ChatChannel> {
        action(
            ctx,
            ChannelRequest::Rename {
                id: id.to_string(),
                name,
            },
        )
        .await
    }

    async fn delete_chat_channel(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        flag(ctx, ChannelRequest::Delete { id: id.to_string() }).await
    }

    async fn leave_chat_channel(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        flag(ctx, ChannelRequest::Leave { id: id.to_string() }).await
    }

    async fn add_chat_channel_member(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<ChatChannel> {
        action(
            ctx,
            ChannelRequest::Invite {
                id: id.to_string(),
                peer: peer_id.to_string(),
            },
        )
        .await
    }

    async fn remove_chat_channel_member(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<ChatChannel> {
        action(
            ctx,
            ChannelRequest::Kick {
                id: id.to_string(),
                peer: peer_id.to_string(),
            },
        )
        .await
    }

    async fn accept_chat_channel_invite(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        flag(ctx, ChannelRequest::Accept { id: id.to_string() }).await
    }

    async fn decline_chat_channel_invite(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        flag(ctx, ChannelRequest::Decline { id: id.to_string() }).await
    }
}

/// The mutations the contract types as `Boolean!` acknowledge the request
/// rather than report a state — a channel that is already gone is the state
/// the caller wanted.
async fn flag(ctx: &Context<'_>, request: ChannelRequest) -> plain_graphql::Result<bool> {
    plain_rs::features::services::chat::channel_runtime::execute(
        ctx.data::<crate::server::ServerState>()?,
        request,
    )
    .await?;
    Ok(true)
}
