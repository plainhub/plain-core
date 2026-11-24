use super::incoming_store as store;
use super::messages::{ChannelMember, MemberSnapshot, State};
use crate::features::chat::enums::DeviceType;
use crate::infrastructure::db::{DChannel, Db};
use crate::infrastructure::transport::contract::signed_request;
use plain_libs::anyhow::{Result, bail};
use plain_libs::serde_json::{Value, json};
use plain_libs::utils::base64::{base64_decode, base64_encode};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Invite,
    StateChanged,
    MemberRemoved,
    Deleted,
    InviteResponse(bool),
    MemberLeft,
}

impl Action {
    pub fn field(self) -> &'static str {
        match self {
            Self::Invite => "notifyChannelInvite",
            Self::StateChanged => "notifyChannelStateChanged",
            Self::MemberRemoved => "notifyChannelMemberRemoved",
            Self::Deleted => "notifyChannelDeleted",
            Self::InviteResponse(_) => "notifyChannelInviteResponse",
            Self::MemberLeft => "notifyChannelMemberLeft",
        }
    }

    pub fn is_owner_action(self) -> bool {
        matches!(
            self,
            Self::Invite | Self::StateChanged | Self::MemberRemoved | Self::Deleted
        )
    }

    pub fn includes_state(self) -> bool {
        matches!(self, Self::Invite | Self::StateChanged)
    }

    pub fn request(
        self,
        channel: &DChannel,
        target: &str,
        state: Option<&State>,
    ) -> Result<Request> {
        let channel_id = &channel.id;
        let (query, variables) = match self {
            Self::Invite => (
                "mutation($channelId: ID!, $input: ChannelInviteInput!) { notifyChannelInvite(channelId: $channelId, input: $input) }",
                json!({"channelId":channel_id,"input":{"channelKey":channel.key,"state":require_state(state)?}}),
            ),
            Self::StateChanged => (
                "mutation($channelId: ID!, $input: ChannelStateInput!) { notifyChannelStateChanged(channelId: $channelId, input: $input) }",
                json!({"channelId":channel_id,"input":require_state(state)?}),
            ),
            Self::MemberRemoved => (
                "mutation($channelId: ID!, $removedPeerId: ID!, $version: Long!) { notifyChannelMemberRemoved(channelId: $channelId, removedPeerId: $removedPeerId, version: $version) }",
                json!({"channelId":channel_id,"removedPeerId":target,"version":channel.version}),
            ),
            Self::Deleted => (
                "mutation($channelId: ID!, $version: Long!) { notifyChannelDeleted(channelId: $channelId, version: $version) }",
                json!({"channelId":channel_id,"version":channel.version}),
            ),
            Self::InviteResponse(accept) => (
                "mutation($channelId: ID!, $accept: Boolean!) { notifyChannelInviteResponse(channelId: $channelId, accept: $accept) }",
                json!({"channelId":channel_id,"accept":accept}),
            ),
            Self::MemberLeft => (
                "mutation($channelId: ID!) { notifyChannelMemberLeft(channelId: $channelId) }",
                json!({"channelId":channel_id}),
            ),
        };
        Ok(Request { query, variables })
    }
}

fn require_state(state: Option<&State>) -> Result<&State> {
    state.ok_or_else(|| plain_libs::anyhow::anyhow!("Missing channel snapshot"))
}

pub struct Request {
    query: &'static str,
    variables: Value,
}

impl Request {
    pub fn sign(&self, keypair: &[u8], channel_id: &str) -> Result<String> {
        signed_request(keypair, self.query, self.variables.clone(), channel_id)
            .map_err(plain_libs::anyhow::Error::msg)
    }
}

pub fn snapshot(
    db: &Db,
    channel: &DChannel,
    actor: &str,
    name: String,
    device_type: DeviceType,
    public_key: &[u8],
    members: Vec<ChannelMember>,
) -> Result<State> {
    let graph_device = |kind| match kind {
        DeviceType::Unknown => DeviceType::Other,
        kind => kind,
    };
    let members = db.with_conn(|c| -> Result<Vec<_>> {
        members
            .into_iter()
            .map(|member| {
                let mut item = MemberSnapshot {
                    peer_id: member.peer_id,
                    status: member.status,
                    name: None,
                    device_type: None,
                    ip: None,
                    public_key: None,
                    port: None,
                };
                if item.peer_id == actor {
                    item.name = Some(name.clone());
                    item.device_type = Some(graph_device(device_type));
                    item.public_key = Some(base64_encode(public_key));
                } else if let Some(peer) = store::peer(c, &item.peer_id)? {
                    item.name = Some(peer.name);
                    item.device_type = Some(graph_device(peer.device_type));
                    item.ip = Some(peer.ip);
                    item.public_key = (!peer.public_key.is_empty()).then_some(peer.public_key);
                    item.port = peer.port;
                }
                Ok(item)
            })
            .collect()
    })?;
    Ok(State {
        name: channel.name.clone(),
        version: channel.version,
        members,
    })
}

pub fn pairing_key(peer: &crate::infrastructure::db::DPeer) -> Result<Vec<u8>> {
    let key = base64_decode(&peer.key);
    if !peer.is_paired() || key.len() != 32 {
        bail!("Channel invitations require a paired peer");
    }
    Ok(key)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/channels/outgoing.rs"]
mod tests;
