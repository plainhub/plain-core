use super::PeerContext;
use crate::graphql::enums::DeviceType;
use crate::graphql::resolvers::chat::ChannelMemberStatus;
use plain_graphql::{Context, ID, Object, SimpleObject};
use plain_rs::features::chat::channels::messages::State;
use plain_rs::features::chat::enums::MemberStatus;
use plain_rs::infrastructure::types::Long;

pub(crate) struct Query;

#[Object]
impl Query {
    fn channel_state(
        &self,
        ctx: &Context<'_>,
        channel_id: ID,
    ) -> plain_graphql::Result<Option<ChannelState>> {
        let c = ctx.data_unchecked::<PeerContext>();
        if channel_id.as_str() != c.channel_id {
            return Err(plain_graphql::Error::new(
                "Channel transport context mismatch",
            ));
        }
        Ok(
            plain_rs::features::services::chat::channel_runtime::owner_snapshot(
                &c.state,
                &c.authenticated.peer.id,
                channel_id.as_str(),
            )?
            .map(Into::into),
        )
    }
}

#[derive(SimpleObject)]
struct ChannelState {
    name: String,
    version: Long,
    members: Vec<ChannelStateMember>,
    signature: String,
}

#[derive(SimpleObject)]
struct ChannelStateMember {
    peer_id: ID,
    status: ChannelMemberStatus,
    name: Option<String>,
    device_type: Option<DeviceType>,
    ip: Option<String>,
    signature_public_key: String,
    encryption_public_key: String,
    port: Option<i32>,
}

impl From<State> for ChannelState {
    fn from(state: State) -> Self {
        Self {
            name: state.name,
            version: Long(state.version),
            members: state
                .members
                .into_iter()
                .map(|member| ChannelStateMember {
                    peer_id: member.peer_id.into(),
                    status: match member.status {
                        MemberStatus::Joined => ChannelMemberStatus::Joined,
                        MemberStatus::Pending => ChannelMemberStatus::Pending,
                    },
                    name: member.name,
                    device_type: member.device_type.map(Into::into),
                    ip: member.ip,
                    signature_public_key: member.signature_public_key,
                    encryption_public_key: member.encryption_public_key,
                    port: member.port.map(i32::from),
                })
                .collect(),
            signature: state.signature,
        }
    }
}
