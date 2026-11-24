use crate::features::chat::attachments::app_file_store::chat_deletion;
use crate::features::chat::channels::messages::{ChannelMember, Notification, State};
use crate::features::chat::channels::outgoing::{self, Action as Outgoing};
use crate::features::chat::channels::state;
use crate::features::chat::channels::state::Action;
use crate::features::services::chat::channel_delivery;
use crate::infrastructure::db::DChannel;
use crate::infrastructure::db::chat_store::channels;
use crate::infrastructure::db::chat_store::peers;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::serde_json::{Value, json};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum Request {
    Snapshot,
    Invitation { id: String, owner: Option<String> },
    Create { name: String },
    Rename { id: String, name: String },
    Delete { id: String },
    Leave { id: String },
    Invite { id: String, peer: String },
    Resend { id: String, peer: String },
    Kick { id: String, peer: String },
    Accept { id: String },
    Decline { id: String },
}
fn with_channels(state: &CoreRuntime, mut result: Value) -> Result<Value> {
    result["channels"] = crate::features::chat::events::public_channels(
        &state.db,
        &state
            .prefs
            .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing client identity"))?,
    )?;
    Ok(result)
}
fn snapshot(state: &CoreRuntime) -> Result<Value> {
    with_channels(
        state,
        json!({"accepted":true,"changed":true,"broadcast":false,"channel":null,"invite":null,"cancel":null}),
    )
}
fn emit(state: &CoreRuntime, result: Value) {
    let _ = state
        .events
        .send(crate::infrastructure::events::WsEvent::broadcast(
            crate::features::chat::events::WS_CHANNELS_UPDATED,
            result.to_string(),
        ));
}
pub fn publish(state: &CoreRuntime, result: Value) -> Result<()> {
    emit(state, with_channels(state, result)?);
    Ok(())
}
fn notify_changed(state: &CoreRuntime) -> Result<()> {
    emit(state, snapshot(state)?);
    Ok(())
}
pub async fn receive(
    state: &CoreRuntime,
    from: &str,
    transport_channel_id: &str,
    channel_id: &str,
    notification: Notification,
) -> Result<()> {
    if !transport_channel_id.is_empty() {
        ensure!(
            transport_channel_id == channel_id,
            "Channel transport context mismatch"
        );
        ensure!(
            !matches!(notification, Notification::Invite(_)),
            "Channel invitations require direct pairing"
        );
    }
    let actor = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|id| !id.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing client identity"))?;
    if let Notification::Invite(roster) | Notification::StateChanged(roster) = &notification
        && let Some(member) = roster.members.iter().find(|member| member.peer_id == actor)
    {
        let signing_key = crate::features::prefs::security::signing_keypair(&state.prefs)?;
        let encryption_key = crate::features::prefs::security::encryption_public_key(&state.prefs)?;
        use crate::infrastructure::db::key_blob;
        ensure!(
            key_blob::encode(&member.signature_public_key)?.as_slice() == &signing_key[32..]
                && key_blob::encode(&member.encryption_public_key)? == encryption_key,
            "Local channel identity mismatch"
        );
    }
    let received = crate::features::chat::channels::incoming::receive(
        &state.db,
        &actor,
        from,
        channel_id,
        notification,
    )?;
    if received.changed || received.invite.is_some() || received.cancel.is_some() {
        publish(state, plain_libs::serde_json::to_value(&received)?)?;
    }
    if received.broadcast {
        if let Some(channel) = received.channel {
            let state = state.clone();
            tokio::spawn(async move {
                if let Err(error) = crate::features::services::chat::channel_delivery::send(
                    &state,
                    &channel,
                    Outgoing::StateChanged,
                    "",
                )
                .await
                {
                    log::warn!("Channel update broadcast: {error}");
                }
            });
        }
    }
    Ok(())
}
pub fn get(state: &CoreRuntime, id: &str) -> Result<DChannel> {
    channels::get(&state.db, id)?.ok_or_else(|| plain_libs::anyhow::anyhow!("Channel not found"))
}

pub fn owner_snapshot(state: &CoreRuntime, member: &str, id: &str) -> Result<Option<State>> {
    let actor = state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?;
    let Some(channel) = channels::get(&state.db, id)? else { return Ok(None) };
    if channel.owner_id.as_deref() != Some(&actor)
        || channel.status != crate::features::chat::enums::ChannelStatus::Joined
    {
        return Ok(None);
    }
    let members: Vec<ChannelMember> = plain_libs::serde_json::from_str(&channel.members)?;
    if !members.iter().any(|entry| entry.peer_id == member) {
        return Ok(None);
    }
    let key = crate::features::prefs::security::signing_keypair(&state.prefs)?;
    Ok(Some(channel_delivery::snapshot(state, &channel, &actor, &key, members)?))
}

pub async fn refresh_from_owners(state: &CoreRuntime) -> Result<()> {
    use futures_util::StreamExt;
    let actor = state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?;
    let mut requests = Vec::new();
    for channel in channels::all(&state.db)? {
        if channel.status == crate::features::chat::enums::ChannelStatus::Joined
            && let Some(owner) = channel.owner_id
            && owner != actor
        {
            requests.push((channel.id, owner, channel.version, channel.updated_at));
        }
    }
    futures_util::stream::iter(requests).for_each_concurrent(4, |(id, owner, version, updated_at)| async move {
        if let Err(error) = refresh_from_owner(state, &id, &owner, version, updated_at).await {
            log::debug!("Channel refresh: {error}");
        }
    }).await;
    Ok(())
}

async fn refresh_from_owner(
    state: &CoreRuntime,
    id: &str,
    owner: &str,
    version: i64,
    updated_at: String,
) -> Result<()> {
    const QUERY: &str = "query($channelId: ID!) { channelState(channelId: $channelId) { name version signature members { peerId status name deviceType ip port signaturePublicKey encryptionPublicKey } } }";
    let Some(peer) = peers::get(&state.db, owner)? else { return Ok(()) };
    let Some(key) = peer.encryption_public_key else { return Ok(()) };
    let request = crate::infrastructure::transport::contract::signed_request(
        &[], QUERY, json!({"channelId": id}), id,
    ).map_err(plain_libs::anyhow::Error::msg)?;
    let response = crate::features::services::peers::peer_transport::send(state, &peer, id, &key, &request)
        .await.map_err(plain_libs::anyhow::Error::msg)?;
    if response.get("errors").is_some_and(|errors| {
        !errors.is_null() && !errors.as_array().is_some_and(Vec::is_empty)
    }) {
        return Ok(());
    }
    let notification = match response["data"].get("channelState") {
        Some(Value::Object(roster)) => Notification::StateChanged(
            plain_libs::serde_json::from_value(Value::Object(roster.clone()))?,
        ),
        Some(Value::Null) => Notification::Unavailable { version, updated_at },
        _ => return Ok(()),
    };
    receive(state, owner, id, id, notification).await
}
async fn send(state: &CoreRuntime, channel: &DChannel, kind: Outgoing, target: &str) -> Value {
    channel_delivery::send(state, channel, kind, target)
        .await
        .unwrap_or_else(channel_delivery::rejected)
}
pub async fn execute(state: &CoreRuntime, request: Request) -> Result<Value> {
    if matches!(&request, Request::Snapshot) {
        return snapshot(state);
    }
    let actor = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing client identity"))?;
    if let Request::Invitation { id, owner } = &request {
        return Ok(
            json!({"channel":state::pending_invitation(&state.db, &actor, id, owner.as_deref())?}),
        );
    }
    let (channel, response) = match request {
        Request::Create { name } => {
            let channel = state::create(&state.db, &actor, &name)?;
            notify_changed(state)?;
            (Some(channel), Value::Null)
        }
        Request::Delete { id } => {
            let channel = chat_deletion::remove_channel(&state.db, &state.directory, &id)?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel not found"))?;
            notify_changed(state)?;
            if channel.owner_id.as_deref() == Some(actor.as_str()) {
                send(state, &channel, Outgoing::Deleted, "").await;
            }
            (None, Value::Null)
        }
        Request::Decline { id } => {
            let channel = get(state, &id)?;
            ensure!(
                peers::get(
                    &state.db,
                    channel
                        .owner_id
                        .as_deref()
                        .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel owner missing"))?
                )?
                .is_some(),
                "Owner peer not found"
            );
            send(
                state,
                &channel,
                Outgoing::InviteResponse(false),
                channel
                    .owner_id
                    .as_deref()
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel owner missing"))?,
            )
            .await;
            chat_deletion::remove_channel_if_matches(&state.db, &state.directory, &channel)?;
            notify_changed(state)?;
            (None, Value::Null)
        }
        request => {
            let (id, action, kind, target, broadcast, changed) = match request {
                Request::Rename { id, name } => {
                    (id, Action::Rename { name }, None, String::new(), true, true)
                }
                Request::Leave { id } => (
                    id,
                    Action::Leave,
                    Some(Outgoing::MemberLeft),
                    String::new(),
                    false,
                    true,
                ),
                Request::Invite { id, peer } => (
                    id,
                    Action::Invite { peer: peer.clone() },
                    Some(Outgoing::Invite),
                    peer,
                    false,
                    true,
                ),
                Request::Resend { id, peer } => (
                    id,
                    Action::Resend { peer: peer.clone() },
                    Some(Outgoing::Invite),
                    peer,
                    false,
                    false,
                ),
                Request::Kick { id, peer } => (
                    id,
                    Action::Kick { peer: peer.clone() },
                    Some(Outgoing::MemberRemoved),
                    peer,
                    true,
                    true,
                ),
                Request::Accept { id } => (
                    id,
                    Action::Accept,
                    Some(Outgoing::InviteResponse(true)),
                    String::new(),
                    false,
                    false,
                ),
                _ => unreachable!(),
            };
            if kind == Some(Outgoing::Invite) {
                let peer = peers::get(&state.db, &target)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Peer not found"))?;
                outgoing::pairing_key(&peer)?;
                ensure!(
                    peer.encryption_public_key.is_some(),
                    "Channel member encryption key unavailable"
                );
            }
            let channel = state::apply(&state.db, &actor, &id, action)?;
            if changed {
                notify_changed(state)?;
            }
            let target = if target.is_empty() {
                channel
                    .owner_id
                    .clone()
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel owner missing"))?
            } else {
                target
            };
            let response = if let Some(kind) = kind {
                send(state, &channel, kind, &target).await
            } else {
                Value::Null
            };
            if broadcast && channel.owner_id.as_deref() == Some(actor.as_str()) {
                send(state, &channel, Outgoing::StateChanged, "").await;
            }
            (channels::get(&state.db, &id)?, response)
        }
    };
    Ok(json!({"channel":channel,"response":response}))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/chat/channel_runtime.rs"]
mod tests;
