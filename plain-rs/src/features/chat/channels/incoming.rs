use crate::features::chat::channels::incoming_store as store;
use crate::features::chat::channels::messages::*;
use crate::features::chat::enums::ChannelStatus;
use crate::features::chat::enums::MemberStatus;
use crate::infrastructure::db::DChannel;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::now_iso;
use plain_libs::anyhow::{Result, bail};

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Received {
    pub accepted: bool,
    pub changed: bool,
    pub broadcast: bool,
    pub channel: Option<DChannel>,
    pub invite: Option<Invite>,
    pub cancel: Option<Cancel>,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub channel_id: String,
    pub channel_name: String,
    pub owner_peer_id: String,
    pub owner_peer_name: String,
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancel {
    pub channel_id: String,
    pub owner_peer_id: String,
}

fn roster(state: &State) -> Result<String> {
    let mut ids = std::collections::HashSet::new();
    let mut members = Vec::with_capacity(state.members.len());
    for member in &state.members {
        if member.peer_id.is_empty() || !ids.insert(&member.peer_id) {
            bail!("Invalid channel member roster");
        }
        members.push(ChannelMember {
            peer_id: member.peer_id.clone(),
            status: member.status,
        });
    }
    Ok(plain_libs::serde_json::to_string(&members)?)
}

fn validate_version(version: i64) -> Result<()> {
    if version < 0 {
        bail!("Invalid channel version");
    }
    Ok(())
}

fn verify_roster(
    state: &State,
    id: &str,
    owner: &crate::infrastructure::db::DPeer,
) -> Result<()> {
    validate_version(state.version)?;
    if !state.members.iter().any(|member| {
        member.peer_id == owner.id
            && member.status == MemberStatus::Joined
            && crate::features::peers::pairing::security::same_public_key(&member.signature_public_key, &owner.public_key)
    }) || !plain_libs::crypto::ed25519_verify(
        &owner.public_key,
        &state.signing_bytes(id),
        &state.signature,
    ) {
        bail!("Invalid channel roster signature");
    }
    Ok(())
}

fn advance(channel: &mut DChannel) -> Result<()> {
    channel.version = channel
        .version
        .checked_add(1)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel version overflow"))?;
    channel.updated_at = now_iso();
    Ok(())
}

pub fn receive(
    db: &Db,
    actor: &str,
    from: &str,
    id: &str,
    notification: Notification,
) -> Result<Received> {
    if actor.is_empty() || from.is_empty() || id.is_empty() {
        bail!("Missing channel message identity");
    }
    if let Notification::MemberRemoved {
        removed_peer_id, ..
    } = &notification
        && removed_peer_id != actor
    {
        bail!("Channel removal recipient mismatch");
    }
    db.with_conn(|c| -> Result<Received> {
        let tx = c.unchecked_transaction()?;
        let current = store::channel(&tx, id)?;
        let mut result = Received {
            accepted: true,
            ..Default::default()
        };
        match notification {
            Notification::Invite(state) => {
                let owner = store::peer(&tx, from)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Unknown invite owner"))?;
                verify_roster(&state, id, &owner)?;
                let members = roster(&state)?;
                if !state
                        .members
                        .iter()
                        .any(|m| m.peer_id == actor && m.status == MemberStatus::Pending)
                {
                    bail!("Invalid invitation membership");
                }
                let new = current.is_none();
                let mut channel = match current {
                    Some(channel) => {
                        if channel.owner_id.as_deref() != Some(from) {
                            bail!("Channel owner mismatch");
                        }
                        if !matches!(channel.status, ChannelStatus::Left | ChannelStatus::Kicked) {
                            return Ok(result);
                        }
                        if state.version <= channel.version {
                            bail!("Stale channel invitation");
                        }
                        channel
                    }
                    None => {
                        let mut channel = DChannel::new(&state.name, from);
                        channel.id = id.into();
                        channel
                    }
                };
                store::ensure_peers(&tx, &state.members, actor)?;
                channel.name = state.name;
                channel.members = members;
                channel.version = state.version;
                channel.status = ChannelStatus::Joined;
                channel.updated_at = now_iso();
                store::save(&tx, &channel, new)?;
                result.invite = Some(Invite {
                    channel_id: id.into(),
                    channel_name: channel.name.clone(),
                    owner_peer_id: from.into(),
                    owner_peer_name: if owner.name.is_empty() {
                        from.into()
                    } else {
                        owner.name
                    },
                });
                result.channel = Some(channel);
            }
            Notification::StateChanged(state) => {
                let mut channel =
                    current.ok_or_else(|| plain_libs::anyhow::anyhow!("Unknown channel"))?;
                if channel.owner_id.as_deref() != Some(from) {
                    bail!("Update sender is not owner");
                }
                let owner = store::peer(&tx, from)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Unknown channel owner"))?;
                verify_roster(&state, id, &owner)?;
                if state.version <= channel.version {
                    return Ok(result);
                }
                let members = roster(&state)?;
                store::ensure_peers(&tx, &state.members, actor)?;
                channel.name = state.name;
                channel.members = members;
                channel.version = state.version;
                channel.updated_at = now_iso();
                store::save(&tx, &channel, false)?;
                result.channel = Some(channel);
            }
            Notification::MemberRemoved { version, .. }
            | Notification::Deleted { version }
            | Notification::Unavailable { version, .. } => {
                validate_version(version)?;
                if let Notification::Unavailable { ref updated_at, .. } = notification
                    && !current.as_ref().is_some_and(|channel| {
                        channel.owner_id.as_deref() == Some(from)
                            && channel.status == ChannelStatus::Joined
                            && channel.version == version
                            && channel.updated_at == *updated_at
                    })
                {
                    return Ok(result);
                }
                if let Some(mut channel) = current {
                    if channel.owner_id.as_deref() != Some(from) {
                        bail!("Kick sender is not owner");
                    }
                    if version < channel.version {
                        return Ok(result);
                    }
                    let mut members = store::members(&channel.members)?;
                    let pending = members.iter().any(|m| m.peer_id == actor && m.is_pending());
                    if channel.status == ChannelStatus::Kicked
                        && !members.iter().any(|m| m.peer_id == actor)
                    {
                        return Ok(result);
                    }
                    members.retain(|m| m.peer_id != actor);
                    channel.members = plain_libs::serde_json::to_string(&members)?;
                    channel.status = ChannelStatus::Kicked;
                    channel.version = version;
                    channel.updated_at = now_iso();
                    store::save(&tx, &channel, false)?;
                    result.channel = Some(channel);
                    if pending {
                        result.cancel = Some(Cancel {
                            channel_id: id.into(),
                            owner_peer_id: from.into(),
                        });
                    }
                } else {
                    result.cancel = Some(Cancel {
                        channel_id: id.into(),
                        owner_peer_id: from.into(),
                    });
                }
            }
            Notification::InviteResponse { .. } | Notification::MemberLeft => {
                let mut channel =
                    current.ok_or_else(|| plain_libs::anyhow::anyhow!("Unknown channel"))?;
                if channel.owner_id.as_deref() != Some(actor) || from == actor {
                    bail!("Invalid member message ownership");
                }
                let mut members = store::members(&channel.members)?;
                let Some(index) = members.iter().position(|m| m.peer_id == from) else {
                    return Ok(result);
                };
                match notification {
                    Notification::InviteResponse { accept } => {
                        if !members[index].is_pending() {
                            return Ok(result);
                        }
                        if accept {
                            members[index].status = MemberStatus::Joined;
                            result.broadcast = true;
                        } else {
                            members.remove(index);
                        }
                    }
                    Notification::MemberLeft => {
                        members.remove(index);
                        result.broadcast = true;
                    }
                    _ => unreachable!(),
                }
                channel.members = plain_libs::serde_json::to_string(&members)?;
                advance(&mut channel)?;
                store::save(&tx, &channel, false)?;
                result.channel = Some(channel);
            }
        }
        result.changed = result.channel.is_some();
        tx.commit()?;
        Ok(result)
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/channels/incoming.rs"]
mod tests;
