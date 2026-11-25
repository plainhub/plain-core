use super::PeerContext;
use crate::graphql::enums::DeviceType;
use crate::graphql::resolvers::chat::{ChannelMemberStatus, ChatItem, item};
use plain_graphql::{Context, ID, InputObject, Object};
use plain_libs::serde_json::json;
use plain_rs::features::chat::channels::messages::{MemberSnapshot, Notification, State};
use plain_rs::features::chat::enums::MemberStatus;
use plain_rs::infrastructure::types::Long;

pub(crate) struct Mutation;

#[Object]
impl Mutation {
    async fn create_chat_item(
        &self,
        ctx: &Context<'_>,
        content: String,
    ) -> plain_graphql::Result<Vec<ChatItem>> {
        let c = ctx.data_unchecked::<PeerContext>();
        plain_libs::serde_json::from_str::<crate::graphql::resolvers::chat::content::ChatContent>(
            &content,
        )?;
        use plain_rs::features::chat::messages::message_lifecycle;
        let message_id = c.authenticated.message_id.as_deref()
            .ok_or_else(|| plain_graphql::Error::new("Missing signed message ID"))?;
        let actor = c.state.native_client_id().map_err(plain_graphql::Error::new)?;
        let received = message_lifecycle::receive(
            &c.state.db, &actor, &c.authenticated.peer.id, &c.channel_id, message_id, &content,
        )?;
        let Some(received) = received else {
            return Ok(vec![]);
        };
        let item = item(&received.chat)?;
        let _ = c
            .state
            .events
            .send(plain_rs::infrastructure::events::WsEvent::broadcast(
                plain_rs::features::chat::events::WS_MESSAGE_CREATED,
                json!([plain_rs::features::chat::service::chat_to_json(
                    &received.chat
                )])
                .to_string(),
            ));
        if let Err(error) = plain_rs::features::services::downloads::chat::received(
            &c.state,
            &received.chat,
            &received.peer.id,
        )
        .await
        {
            log::warn!("Incoming attachment queue: {error}");
        }
        c.state.previews.request(&received.chat.id);
        Ok(vec![item])
    }
    async fn notify_channel_invite(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
        input: ChannelStateInput,
    ) -> plain_graphql::Result<bool> {
        receive(ctx, channel_id, Notification::Invite(input.try_into()?)).await
    }

    async fn notify_channel_state_changed(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
        input: ChannelStateInput,
    ) -> plain_graphql::Result<bool> {
        receive(
            ctx,
            channel_id,
            Notification::StateChanged(input.try_into()?),
        )
        .await
    }

    async fn notify_channel_member_removed(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
        removed_peer_id: ID,
        version: Long,
    ) -> plain_graphql::Result<bool> {
        receive(
            ctx,
            channel_id,
            Notification::MemberRemoved {
                removed_peer_id: removed_peer_id.to_string(),
                version: version.0,
            },
        )
        .await
    }

    async fn notify_channel_deleted(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
        version: Long,
    ) -> plain_graphql::Result<bool> {
        receive(
            ctx,
            channel_id,
            Notification::Deleted { version: version.0 },
        )
        .await
    }

    async fn notify_channel_invite_response(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
        accept: bool,
    ) -> plain_graphql::Result<bool> {
        receive(ctx, channel_id, Notification::InviteResponse { accept }).await
    }

    async fn notify_channel_member_left(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
    ) -> plain_graphql::Result<bool> {
        receive(ctx, channel_id, Notification::MemberLeft).await
    }
}

async fn receive(
    ctx: &Context<'_>,
    channel_id: ID,
    notification: Notification,
) -> plain_graphql::Result<bool> {
    let c = ctx.data_unchecked::<PeerContext>();
    plain_rs::features::services::chat::channel_runtime::receive(
        &c.state,
        &c.authenticated.peer.id,
        &c.channel_id,
        &channel_id,
        notification,
    )
    .await?;
    Ok(true)
}

#[derive(InputObject)]
struct ChannelStateInput {
    name: String,
    version: Long,
    members: Vec<ChannelMemberInput>,
    signature: String,
}

#[derive(InputObject)]
struct ChannelMemberInput {
    peer_id: ID,
    status: ChannelMemberStatus,
    name: Option<String>,
    device_type: Option<DeviceType>,
    ip: Option<String>,
    signature_public_key: String,
    encryption_public_key: String,
    port: Option<i32>,
}

impl TryFrom<ChannelStateInput> for State {
    type Error = plain_graphql::Error;

    fn try_from(input: ChannelStateInput) -> Result<Self, Self::Error> {
        let members = input
            .members
            .into_iter()
            .map(|member| {
                let port = member
                    .port
                    .map(u16::try_from)
                    .transpose()
                    .map_err(|_| plain_graphql::Error::new("Invalid channel member port"))?;
                Ok::<_, Self::Error>(MemberSnapshot {
                    peer_id: member.peer_id.to_string(),
                    status: match member.status {
                        ChannelMemberStatus::Joined => MemberStatus::Joined,
                        ChannelMemberStatus::Pending => MemberStatus::Pending,
                    },
                    name: member.name,
                    device_type: member.device_type.map(Into::into),
                    ip: member.ip,
                    signature_public_key: member.signature_public_key,
                    encryption_public_key: member.encryption_public_key,
                    port,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            name: input.name,
            version: input.version.0,
            members,
            signature: input.signature,
        })
    }
}
