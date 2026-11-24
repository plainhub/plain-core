use crate::features::chat::channels::messages::{ChannelMember, State};
use crate::features::chat::channels::outgoing::{self, Action};
use crate::infrastructure::db::DChannel;
use crate::infrastructure::db::chat_store::channels;
use crate::infrastructure::db::chat_store::peers;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::serde_json::{Value, json};

pub fn rejected(message: impl ToString) -> Value {
    json!({"errors":[{"message":message.to_string()}]})
}
fn current(state: &CoreRuntime, channel: &DChannel, removed: bool) -> Result<bool> {
    let row = channels::get(&state.db, &channel.id)?;
    Ok(if removed {
        row.is_none()
    } else {
        row.as_ref() == Some(channel)
    })
}

pub(super) fn snapshot(
    state: &CoreRuntime,
    channel: &DChannel,
    actor: &str,
    keypair: &[u8],
    members: Vec<ChannelMember>,
) -> Result<State> {
    outgoing::snapshot(
        &state.db,
        channel,
        actor,
        crate::features::prefs::settings::snapshot(&state.prefs)?.device_name,
        crate::features::services::discovery::discovery_advertisement::device_type(state),
        keypair,
        &crate::features::prefs::security::encryption_public_key(&state.prefs)?,
        members,
    )
}

pub async fn send(
    state: &CoreRuntime,
    channel: &DChannel,
    action: Action,
    target: &str,
) -> Result<Value> {
    let mut stop = state.stop.clone();
    ensure!(!*stop.borrow(), "Server stopped");
    tokio::select! {
        biased;
        _ = stop.changed() => Ok(rejected("Server stopped")),
        result = deliver(state, channel, action, target) => result,
    }
}
async fn deliver(
    state: &CoreRuntime,
    channel: &DChannel,
    action: Action,
    target: &str,
) -> Result<Value> {
    let removed = action == Action::Deleted;
    ensure!(
        current(state, channel, removed)?,
        "Channel changed before send"
    );
    let actor = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|id| !id.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing client identity"))?;
    if action.is_owner_action() {
        ensure!(
            channel.owner_id.as_deref() == Some(&actor),
            "Only owner can send channel updates"
        );
    } else {
        ensure!(
            !target.is_empty() && Some(target) == channel.owner_id.as_deref(),
            "Invalid channel owner destination"
        );
    }
    if matches!(action, Action::Invite | Action::MemberRemoved) {
        ensure!(!target.is_empty(), "Missing channel notification recipient");
    }

    let members: Vec<ChannelMember> = plain_libs::serde_json::from_str(&channel.members)?;
    let mut targets = Vec::new();
    if target.is_empty() {
        for member in members.iter().filter(|member| member.peer_id != actor) {
            if peers::get(&state.db, &member.peer_id)?.is_some() {
                targets.push(member.peer_id.clone());
            }
        }
    } else if peers::get(&state.db, target)?.is_some() {
        targets.push(target.into());
    } else {
        return Ok(rejected("Peer not found"));
    }
    let field = action.field();
    let mut response = json!({"data":{(field):true}});
    if targets.is_empty() {
        return Ok(response);
    }
    let keypair = crate::features::prefs::security::signing_keypair(&state.prefs)?;
    let snapshot = if action.includes_state() {
        Some(snapshot(state, channel, &actor, &keypair, members)?)
    } else {
        None
    };
    let request = action.request(channel, target, snapshot.as_ref())?;
    let direct = action == Action::Invite;
    let cid = if direct { "" } else { channel.id.as_str() };
    let wire = tokio::sync::OnceCell::<String>::new();
    for group in targets.chunks(4) {
        let results = futures_util::future::join_all(group.iter().map(|target| async {
            let _permit = state
                .channel_delivery
                .acquire()
                .await
                .map_err(|e| plain_libs::anyhow::anyhow!(e))?;
            ensure!(
                current(state, channel, removed)?,
                "Channel changed before send"
            );
            let peer = peers::get(&state.db, target)?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Peer not found"))?;
            let key = if direct {
                outgoing::pairing_key(&peer)?
            } else {
                let public_key = peer.encryption_public_key
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Channel member encryption key unavailable"))?;
                public_key.to_vec()
            };
            let wire = wire.get_or_try_init(|| async { request.sign(&keypair, cid) }).await?;
            let response = crate::features::services::peers::peer_transport::send_checked(
                state,
                &peer,
                cid,
                &key,
                wire,
                || {
                    if !current(state, channel, removed).map_err(|e| e.to_string())? {
                        return Err("Channel changed before send".into());
                    }
                    let latest = peers::get(&state.db, &peer.id)
                        .map_err(|e| e.to_string())?
                        .ok_or("Peer not found")?;
                    if !crate::features::services::peers::peer_address::same_identity(
                        &latest, &peer,
                    ) {
                        return Err("Peer changed before send".into());
                    }
                    Ok(())
                },
            )
            .await;
            Ok::<Value, plain_libs::anyhow::Error>(match response {
                Ok(value)
                    if value["data"][field].as_bool() == Some(true)
                        && value.get("errors").is_none_or(|v| {
                            v.is_null() || v.as_array().is_some_and(Vec::is_empty)
                        }) =>
                {
                    value
                }
                Ok(value)
                    if value
                        .get("errors")
                        .is_some_and(|v| v.as_array().is_some_and(|v| !v.is_empty())) =>
                {
                    value
                }
                Ok(_) => rejected("Channel message rejected"),
                Err(error) => rejected(error),
            })
        }))
        .await;
        for result in results {
            let result = result.unwrap_or_else(rejected);
            if result.get("errors").is_some() {
                response = result;
            }
        }
    }
    Ok(response)
}
