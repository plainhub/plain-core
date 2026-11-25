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
                "mutation($channelId: ID!, $input: ChannelStateInput!) { notifyChannelInvite(channelId: $channelId, input: $input) }",
                json!({"channelId":channel_id,"input":require_state(state)?}),
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
    signing_keypair: &[u8],
    encryption_public_key: &[u8; 32],
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
                if member.peer_id == actor {
                    return Ok(MemberSnapshot {
                        peer_id: member.peer_id,
                        status: member.status,
                        name: Some(name.clone()),
                        device_type: Some(graph_device(device_type)),
                        ip: None,
                        signature_public_key: base64_encode(&signing_keypair[32..]),
                        encryption_public_key: base64_encode(encryption_public_key),
                        port: None,
                    });
                }
                let peer = store::peer(c, &member.peer_id)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel member identity unavailable"))?;
                let key = peer.encryption_public_key
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel member encryption key unavailable"))?;
                store::public_key(&peer.public_key)?;
                Ok(MemberSnapshot {
                    peer_id: member.peer_id,
                    status: member.status,
                    name: Some(peer.name),
                    device_type: Some(graph_device(peer.device_type)),
                    ip: Some(peer.ip),
                    signature_public_key: peer.public_key,
                    encryption_public_key: base64_encode(&key),
                    port: peer.port,
                })
            })
            .collect()
    })?;
    let mut state = State {
        name: channel.name.clone(),
        version: channel.version,
        members,
        signature: String::new(),
    };
    state.signature = plain_libs::crypto::ed25519_sign(
        signing_keypair,
        &state.signing_bytes(&channel.id),
    );
    Ok(state)
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
