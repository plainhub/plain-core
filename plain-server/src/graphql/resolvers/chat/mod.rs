pub mod content;
pub mod host;

pub mod inputs;
pub mod mutation;

pub mod query;

use self::content::{ChatContent, ChatDeliveryResult};
use plain_graphql::{Context, Enum, SimpleObject};
use plain_libs::serde_json::Value;
pub use plain_rs::features::chat::enums::ChatStatus;
use plain_rs::features::services::chat::channel_runtime::Request as ChannelRequest;
use plain_rs::infrastructure::db::{DChat, Db};
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

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
    pub peer_id: plain_graphql::ID,
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
pub struct ChatChannel {
    pub actions: ChatChannelActions,
    pub id: plain_graphql::ID,
    #[graphql(name = "ownerId")]
    pub owner_id: plain_graphql::ID,
    pub name: String,
    pub members: Vec<ChatChannelMember>,
    /// Monotonically increasing mutation counter; receivers ignore channel
    /// updates whose version is not greater than their local copy.
    pub version: plain_rs::infrastructure::types::Long,
    pub status: ChatChannelStatus,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
}


#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatItem {
    /// `me` when this device created the item, otherwise the client id of
    /// the originating peer.
    #[graphql(name = "fromId")]
    pub from_id: plain_graphql::ID,
    /// Peer id for direct messages; empty for channel messages —
    /// `channelId` is authoritative there.
    #[graphql(name = "toId")]
    pub to_id: plain_graphql::ID,
    /// Null for direct messages: the store keeps "" and the contract says
    /// null, because "" is not a channel a client can query.
    #[graphql(name = "channelId")]
    pub channel_id: Option<plain_graphql::ID>,
    pub id: plain_graphql::ID,
    #[serde(deserialize_with = "self::content::read_content")]
    pub content: ChatContent,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
    pub status: ChatStatus,
    #[serde(
        rename = "statusData",
        deserialize_with = "self::content::read_deliveries"
    )]
    pub delivery_results: Vec<ChatDeliveryResult>,
}

async fn action(ctx: &Context<'_>, request: ChannelRequest) -> plain_graphql::Result<ChatChannel> {
    let result = plain_rs::features::services::chat::channel_runtime::execute(
        ctx.data::<crate::server::ServerState>()?,
        request,
    )
    .await?;
    let row: plain_rs::infrastructure::db::DChannel =
        plain_libs::serde_json::from_value(result["channel"].clone())?;
    Ok(channel(&plain_rs::features::chat::events::channel_to_json(
        &row,
        &actor(ctx)?,
    )))
}

pub(crate) fn item(row: &DChat) -> plain_graphql::Result<ChatItem> {
    Ok(plain_libs::serde_json::from_value(
        plain_rs::features::chat::service::chat_to_json(row),
    )?)
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
        version: plain_rs::infrastructure::types::Long(crate::graphql::types::facts::integer(
            value, "version",
        )),
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

fn rows<T>(value: &Value, item: impl Fn(&Value) -> T) -> Vec<T> {
    crate::graphql::types::facts::rows(value, item)
}

fn db<'a>(ctx: &'a Context<'_>) -> plain_graphql::Result<&'a Arc<Db>> {
    ctx.data::<Arc<Db>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/chat/mod.rs"]
mod tests;

fn actor(ctx: &Context<'_>) -> plain_graphql::Result<String> {
    ctx.data::<Arc<plain_rs::features::prefs::Prefs>>()?
        .get::<String>(plain_rs::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|actor| !actor.is_empty())
        .ok_or_else(|| plain_graphql::Error::new("Missing client identity"))
}

fn has_channel_target(targets: &[String]) -> bool {
    targets.iter().any(|target| {
        plain_rs::features::chat::messages::target::parse(target)
            .is_ok_and(|(_, channel)| !channel.is_empty())
    })
}

fn authorize_attachment_import(ctx: &plain_graphql::Context<'_>, content: &content::ChatContent) -> plain_graphql::Result<()> {
    if let content::ChatContent::Message(message) = content
        && message.items.iter().any(|item| !item.attachment.uri.starts_with("fid:"))
    {
        crate::http::auth::public_gate::require(
            ctx, &ctx.data::<crate::server::ServerState>()?.prefs,
            &[plain_rs::features::access::AccessFeature::Files],
        )?;
    }
    Ok(())
}

fn authorize_stored_attachment_import(
    ctx: &plain_graphql::Context<'_>, id: &str, targets: Option<&[String]>,
) -> plain_graphql::Result<()> {
    if crate::http::auth::public_gate::is_local(ctx)
        || targets.is_some_and(|targets| !has_channel_target(targets))
    {
        return Ok(());
    }
    let db = &ctx.data::<crate::server::ServerState>()?.db;
    if let Some(chat) = plain_rs::infrastructure::db::chat_store::messages::get(db, id)?
        && (targets.is_some() || chat.channel_id.is_some())
    {
        authorize_attachment_import(ctx, &plain_libs::serde_json::from_str(&chat.content)?)?;
    }
    Ok(())
}
