use anyhow::{Result, ensure};
use plain_rs::chat::channels::messages::ChannelMember;
use plain_rs::chat::enums::ChannelSystemMessageType;
use plain_rs::chat::enums::DeviceType;
use plain_rs::db::DChannel;
use plain_rs::db::chat_store::channels;
use plain_rs::db::chat_store::peers;
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::{Value, json};

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
pub async fn send(
    state: &CoreRuntime,
    channel: &DChannel,
    kind: ChannelSystemMessageType,
    target: &str,
    removed: bool,
) -> Result<Value> {
    let mut stop = state.stop.clone();
    ensure!(!*stop.borrow(), "Server stopped");
    tokio::select! {
        biased;
        _ = stop.changed() => Ok(rejected("Server stopped")),
        result = deliver(state, channel, kind, target, removed) => result,
    }
}
async fn deliver(
    state: &CoreRuntime,
    channel: &DChannel,
    kind: ChannelSystemMessageType,
    target: &str,
    removed: bool,
) -> Result<Value> {
    ensure!(
        current(state, channel, removed)?,
        "Channel changed before send"
    );
    let actor = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .unwrap_or_default();
    ensure!(!actor.is_empty(), "Missing client identity");
    match kind {
        ChannelSystemMessageType::Invite
        | ChannelSystemMessageType::Update
        | ChannelSystemMessageType::Kick => ensure!(
            channel.owner_id.as_deref() == Some(actor.as_str()),
            "Only owner can send channel updates"
        ),
        _ => ensure!(
            !target.is_empty() && Some(target) == channel.owner_id.as_deref(),
            "Invalid channel owner destination"
        ),
    }
    ensure!(
        kind != ChannelSystemMessageType::Invite || !target.is_empty(),
        "Missing invite target"
    );
    let ids = if target.is_empty() {
        serde_json::from_str::<Vec<ChannelMember>>(&channel.members)?
            .into_iter()
            .filter(|m| m.peer_id != actor)
            .map(|m| m.peer_id)
            .collect::<Vec<_>>()
    } else {
        vec![target.to_owned()]
    };
    let mut present = false;
    for id in &ids {
        present |= peers::get(&state.db, id)?.is_some();
    }
    if !present {
        return Ok(if target.is_empty() {
            json!({"data":{"channelSystemMessage":true}})
        } else {
            rejected("Peer not found")
        });
    }
    struct Device {
        name: String,
        device_type: DeviceType,
    }
    let device = if matches!(
        kind,
        ChannelSystemMessageType::Invite
            | ChannelSystemMessageType::Update
            | ChannelSystemMessageType::InviteAccept
    ) {
        Device {
            name: plain_rs::prefs::settings::snapshot(&state.prefs)?.device_name,
            device_type: plain_rs::services::discovery::discovery_advertisement::device_type(state),
        }
    } else {
        Device {
            name: String::new(),
            device_type: DeviceType::Unknown,
        }
    };
    ensure!(
        current(state, channel, removed)?,
        "Channel changed before send"
    );
    let prepared = plain_rs::services::chat::channel_outgoing::prepare(
        &state.db,
        &state.prefs,
        channel,
        kind,
        target,
        &device.name,
        device.device_type,
    )?;
    let direct_wire = tokio::sync::OnceCell::<String>::new();
    let channel_wire = tokio::sync::OnceCell::<String>::new();
    let mut response = json!({"data":{"channelSystemMessage":true}});
    for group in prepared.targets.chunks(4) {
        let results = futures_util::future::join_all(group.iter().map(|target| async {
            let _permit = state
                .channel_delivery
                .acquire()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            ensure!(
                current(state, channel, removed)?,
                "Channel changed before send"
            );
            let peer = peers::get(&state.db, &target.peer.id)?
                .ok_or_else(|| anyhow::anyhow!("Peer not found"))?;
            let (key, cid) = if peer.key.is_empty() {
                (&channel.key, channel.id.as_str())
            } else {
                (&peer.key, "")
            };
            let wire = if cid.is_empty() {
                &direct_wire
            } else {
                &channel_wire
            }
            .get_or_try_init(|| async {
                plain_rs::services::chat::channel_outgoing::wire(
                    &state.prefs,
                    kind,
                    &prepared.payload,
                    cid,
                )
            })
            .await?;
            let response = plain_rs::services::peers::peer_transport::send_checked(
                state,
                &peer,
                cid,
                &plain_rs::utils::base64::base64_decode(key),
                wire,
                || {
                    if !current(state, channel, removed).map_err(|e| e.to_string())? {
                        return Err("Channel changed before send".into());
                    }
                    let latest = peers::get(&state.db, &peer.id)
                        .map_err(|e| e.to_string())?
                        .ok_or("Peer not found")?;
                    if !plain_rs::services::peers::peer_address::same_identity(&latest, &peer) {
                        return Err("Peer changed before send".into());
                    }
                    Ok(())
                },
            )
            .await;
            Ok::<Value, anyhow::Error>(match response {
                Ok(value)
                    if value["data"]["channelSystemMessage"].as_bool() == Some(true)
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
