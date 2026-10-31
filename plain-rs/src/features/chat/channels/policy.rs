use crate::features::chat::enums::ChannelStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actions {
    pub is_owner: bool,
    pub can_rename: bool,
    pub can_manage_members: bool,
    pub can_leave: bool,
    pub can_delete: bool,
}

pub fn actions(owner: Option<&str>, status: ChannelStatus, actor: &str) -> Actions {
    let is_owner = !actor.is_empty() && owner == Some(actor);
    Actions {
        is_owner,
        can_rename: is_owner,
        can_manage_members: is_owner,
        can_leave: !actor.is_empty() && !is_owner && status == ChannelStatus::Joined,
        can_delete: !actor.is_empty()
            && (is_owner || matches!(status, ChannelStatus::Left | ChannelStatus::Kicked)),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/channels/policy.rs"]
mod tests;
