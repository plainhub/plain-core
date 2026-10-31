use crate::graphql::types::ActionResult;
use async_graphql::{Context, Enum, Object, SimpleObject};
use plain_rs::db::DChat;
use plain_rs::db::Db;
use plain_rs::types::Instant;
use serde_json::{Value, json};
use std::sync::Arc;

pub use plain_rs::chat::enums::ChatStatus;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum ChatChannelStatus {
    Joined,
    Left,
    Kicked,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChannelMemberStatus {
    Joined,
    Pending,
}

#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatChannelMember {
    #[graphql(name = "peerId")]
    pub peer_id: async_graphql::ID,
    pub status: ChannelMemberStatus,
}

#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatChannelActions {
    pub is_owner: bool,
    pub can_rename: bool,
    pub can_manage_members: bool,
    pub can_leave: bool,
    pub can_delete: bool,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct ChatChannel {
    pub actions: ChatChannelActions,
    pub id: async_graphql::ID,
    #[graphql(name = "ownerId")]
    pub owner_id: async_graphql::ID,
    pub name: String,
    pub members: Vec<ChatChannelMember>,
    /// Monotonically increasing mutation counter; receivers ignore channel
    /// updates whose version is not greater than their local copy.
    pub version: plain_rs::types::Long,
    pub status: ChatChannelStatus,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
}

#[async_graphql::ComplexObject]
impl ChatChannel {
    #[graphql(guard = "crate::http::auth::public_gate::LocalOwner")]
    async fn key(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        let db = db(ctx)?.clone();
        let id = self.id.clone();
        let row =
            tokio::task::spawn_blocking(move || plain_rs::db::chat_store::channels::get(&db, &id))
                .await??;
        Ok(row
            .ok_or_else(|| async_graphql::Error::new("Chat channel no longer exists"))?
            .key)
    }
}

#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatItem {
    /// `me` when this device created the item, otherwise the client id of
    /// the originating peer.
    #[graphql(name = "fromId")]
    pub from_id: async_graphql::ID,
    /// Peer id for direct messages; empty for channel messages —
    /// `channelId` is authoritative there.
    #[graphql(name = "toId")]
    pub to_id: async_graphql::ID,
    /// Null for direct messages: the store keeps "" and the contract says
    /// null, because "" is not a channel a client can query.
    #[graphql(name = "channelId")]
    pub channel_id: Option<async_graphql::ID>,
    pub id: async_graphql::ID,
    /// Message envelope JSON: `{type: TEXT|IMAGES|FILES|SHARE, value: {…}}` —
    /// parse `value` according to `type`.
    pub content: String,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
    pub status: ChatStatus,
    /// Per-recipient delivery details as JSON (peer delivery results);
    /// empty when the message has no delivery failures. Drives
    /// SENT/PARTIAL/FAILED alongside `status`.
    #[graphql(name = "statusData")]
    pub status_data: String,
}

#[derive(Default)]
pub struct ChatQuery;

#[Object]
impl ChatQuery {
    /// Latest-first page of one conversation, returned oldest-to-newest so
    /// clients can render directly. `target` is the chat target id: a bare
    /// peer id (an optional `peer:` prefix is accepted), or a
    /// `channel:<id>` prefixed channel id.
    async fn chat_items(
        &self,
        ctx: &Context<'_>,
        target: String,
        query: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        let text = text_of(&query);
        let rows = db(ctx)?.get_chats_page(&target, &text, offset, limit);
        if let Some(state) = ctx.data_opt::<crate::server::ServerState>() {
            plain_rs::services::chat::prewarm::prepare_page(state, &target, &text, offset, limit);
        }
        Ok(rows.iter().map(item).collect())
    }

    async fn chat_item(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<Option<ChatItem>> {
        Ok(
            plain_rs::db::chat_store::messages::get(db(ctx)?, id.as_str())?
                .as_ref()
                .map(item),
        )
    }

    async fn chat_channels(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<ChatChannel>> {
        let facts = plain_rs::chat::events::public_channels(db(ctx)?, &actor(ctx)?)?;
        Ok(rows(&facts, channel))
    }
}

#[derive(Default)]
pub struct ChatMutation;

#[Object]
impl ChatMutation {
    /// Send a chat message. `target` is the chat target id — a bare peer id
    /// (an optional `peer:` prefix is accepted), or a `channel:<id>`
    /// prefixed channel id; same value space as the `chatItems` query.
    /// `content` is the message envelope JSON, same shape as
    /// `ChatItem.content`.
    async fn send_chat_item(
        &self,
        ctx: &Context<'_>,
        target: String,
        content: String,
    ) -> async_graphql::Result<Vec<ChatItem>> {
        let row = plain_rs::services::chat::chat_actions::send(
            ctx.data::<crate::server::ServerState>()?,
            &target,
            &content,
        )
        .await?;
        Ok(vec![item(&row)])
    }

    /// Always true: an id that is already gone is the state the caller asked
    /// for, and the contract has no way to say "nothing was there".
    async fn delete_chat_item(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        plain_rs::services::chat::chat_actions::delete(
            ctx.data::<crate::server::ServerState>()?,
            vec![id.to_string()],
        )?;
        Ok(true)
    }

    async fn delete_chat_items(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<ActionResult> {
        let count = plain_rs::services::chat::chat_actions::delete_query(
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
        id: async_graphql::ID,
    ) -> async_graphql::Result<ChatItem> {
        let row = plain_rs::services::chat::chat_actions::retry(
            ctx.data::<crate::server::ServerState>()?,
            id.to_string(),
        )
        .await?;
        Ok(item(&row))
    }

    async fn create_chat_channel(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> async_graphql::Result<ChatChannel> {
        action(ctx, "create", &[("name", json!(name))]).await
    }

    async fn update_chat_channel(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        name: String,
    ) -> async_graphql::Result<ChatChannel> {
        action(
            ctx,
            "rename",
            &[("id", json!(id.as_str())), ("name", json!(name))],
        )
        .await
    }

    async fn delete_chat_channel(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        flag(ctx, "delete", &[("id", json!(id.as_str()))]).await
    }

    async fn leave_chat_channel(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        flag(ctx, "leave", &[("id", json!(id.as_str()))]).await
    }

    async fn add_chat_channel_member(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        peer_id: async_graphql::ID,
    ) -> async_graphql::Result<ChatChannel> {
        action(
            ctx,
            "invite",
            &[
                ("id", json!(id.as_str())),
                ("peer", json!(peer_id.as_str())),
            ],
        )
        .await
    }

    async fn remove_chat_channel_member(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        peer_id: async_graphql::ID,
    ) -> async_graphql::Result<ChatChannel> {
        action(
            ctx,
            "kick",
            &[
                ("id", json!(id.as_str())),
                ("peer", json!(peer_id.as_str())),
            ],
        )
        .await
    }

    async fn accept_chat_channel_invite(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        flag(ctx, "accept", &[("id", json!(id.as_str()))]).await
    }

    async fn decline_chat_channel_invite(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        flag(ctx, "decline", &[("id", json!(id.as_str()))]).await
    }
}

async fn action(
    ctx: &Context<'_>,
    action: &str,
    fields: &[(&str, Value)],
) -> async_graphql::Result<ChatChannel> {
    let mut params = serde_json::Map::new();
    params.insert("action".to_string(), json!(action));
    for (key, value) in fields {
        params.insert((*key).to_string(), value.clone());
    }
    let request = serde_json::from_value(Value::Object(params))?;
    let result = plain_rs::services::chat::channel_runtime::execute(
        ctx.data::<crate::server::ServerState>()?,
        request,
    )
    .await?;
    let row: plain_rs::db::DChannel = serde_json::from_value(result["channel"].clone())?;
    Ok(channel(&plain_rs::chat::events::channel_to_json(
        &row,
        &actor(ctx)?,
    )))
}

/// The mutations the contract types as `Boolean!` acknowledge the request
/// rather than report a state — a channel that is already gone is the state
/// the caller wanted.
async fn flag(
    ctx: &Context<'_>,
    action: &str,
    fields: &[(&str, Value)],
) -> async_graphql::Result<bool> {
    let mut params = serde_json::Map::new();
    params.insert("action".to_string(), json!(action));
    for (key, value) in fields {
        params.insert((*key).to_string(), value.clone());
    }
    let request = serde_json::from_value(Value::Object(params))?;
    plain_rs::services::chat::channel_runtime::execute(
        ctx.data::<crate::server::ServerState>()?,
        request,
    )
    .await?;
    Ok(true)
}

pub(crate) fn item(row: &DChat) -> ChatItem {
    from_json(&plain_rs::chat::service::chat_to_json(row))
}

fn from_json(value: &Value) -> ChatItem {
    ChatItem {
        from_id: crate::graphql::types::facts::id(value, "fromId"),
        to_id: crate::graphql::types::facts::id(value, "toId"),
        channel_id: Some(crate::graphql::types::facts::id(value, "channelId"))
            .filter(|id| !id.is_empty()),
        id: crate::graphql::types::facts::id(value, "id"),
        content: crate::graphql::types::facts::text(value, "content"),
        created_at: crate::graphql::types::facts::stored_instant(
            &crate::graphql::types::facts::text(value, "createdAt"),
        ),
        updated_at: crate::graphql::types::facts::stored_instant(
            &crate::graphql::types::facts::text(value, "updatedAt"),
        ),
        status: crate::graphql::types::facts::text(value, "status")
            .parse()
            .unwrap_or(ChatStatus::Sent),
        status_data: crate::graphql::types::facts::text(value, "statusData"),
    }
}

fn channel(value: &Value) -> ChatChannel {
    ChatChannel {
        actions: ChatChannelActions {
            is_owner: crate::graphql::types::facts::flag(&value["actions"], "isOwner"),
            can_rename: crate::graphql::types::facts::flag(&value["actions"], "canRename"),
            can_manage_members: crate::graphql::types::facts::flag(
                &value["actions"],
                "canManageMembers",
            ),
            can_leave: crate::graphql::types::facts::flag(&value["actions"], "canLeave"),
            can_delete: crate::graphql::types::facts::flag(&value["actions"], "canDelete"),
        },
        id: crate::graphql::types::facts::id(value, "id"),
        owner_id: crate::graphql::types::facts::id(value, "ownerId"),
        name: crate::graphql::types::facts::text(value, "name"),
        members: crate::graphql::types::facts::list(value, "members", |member| ChatChannelMember {
            peer_id: crate::graphql::types::facts::id(member, "peerId"),
            status: match crate::graphql::types::facts::text(member, "status").as_str() {
                "PENDING" => ChannelMemberStatus::Pending,
                _ => ChannelMemberStatus::Joined,
            },
        }),
        version: plain_rs::types::Long(crate::graphql::types::facts::integer(value, "version")),
        status: match crate::graphql::types::facts::text(value, "status").as_str() {
            "LEFT" => ChatChannelStatus::Left,
            "KICKED" => ChatChannelStatus::Kicked,
            _ => ChatChannelStatus::Joined,
        },
        created_at: crate::graphql::types::facts::stored_instant(
            &crate::graphql::types::facts::text(value, "createdAt"),
        ),
        updated_at: crate::graphql::types::facts::stored_instant(
            &crate::graphql::types::facts::text(value, "updatedAt"),
        ),
    }
}

fn text_of(query: &str) -> String {
    plain_rs::utils::search_dsl::parse(query)
        .into_iter()
        .find(|field| field.name == "text")
        .map(|field| field.value)
        .unwrap_or_default()
}

fn rows<T>(value: &Value, item: impl Fn(&Value) -> T) -> Vec<T> {
    crate::graphql::types::facts::rows(value, item)
}

fn db<'a>(ctx: &'a Context<'_>) -> async_graphql::Result<&'a Arc<Db>> {
    ctx.data::<Arc<Db>>()
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/chat/public.rs"]
mod tests;

fn actor(ctx: &Context<'_>) -> async_graphql::Result<String> {
    ctx.data::<Arc<plain_rs::prefs::Prefs>>()?
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .filter(|actor| !actor.is_empty())
        .ok_or_else(|| async_graphql::Error::new("Missing client identity"))
}
