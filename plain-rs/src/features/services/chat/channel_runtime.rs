use crate::features::chat::attachments::app_file_store::chat_deletion;
use crate::features::chat::channels::messages::Notification;
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
            !matches!(notification, Notification::Invite { .. }),
            "Channel invitations require direct pairing"
        );
    }
    let actor = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|id| !id.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing client identity"))?;
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
