use crate::features::chat::enums::{DeviceType, MemberStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMember {
    /// Wire/storage key is `peerId` since the 2026-09-24 naming cleanup; the
    /// legacy `id` key stays decodable via the serde alias (older app versions).
    #[serde(rename = "peerId", alias = "id")]
    pub peer_id: String,
    #[serde(default)]
    pub status: MemberStatus,
}

impl ChannelMember {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            peer_id: id.into(),
            status: MemberStatus::Joined,
        }
    }

    pub fn pending(id: impl Into<String>) -> Self {
        Self {
            peer_id: id.into(),
            status: MemberStatus::Pending,
        }
    }

    pub fn is_joined(&self) -> bool {
        self.status == MemberStatus::Joined
    }

    pub fn is_pending(&self) -> bool {
        self.status == MemberStatus::Pending
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberSnapshot {
    pub peer_id: String,
    pub status: MemberStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_type: Option<DeviceType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub name: String,
    pub version: i64,
    pub members: Vec<MemberSnapshot>,
}

pub enum Notification {
    Invite {
        channel_key: String,
        state: State,
    },
    StateChanged(State),
    MemberRemoved {
        removed_peer_id: String,
        version: i64,
    },
    Deleted {
        version: i64,
    },
    InviteResponse {
        accept: bool,
    },
    MemberLeft,
}

/// Parse the `members` storage string into a typed roster.
pub fn decode_members(raw: &str) -> Vec<ChannelMember> {
    plain_libs::serde_json::from_str(raw).unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/channels/messages.rs"]
mod tests;
