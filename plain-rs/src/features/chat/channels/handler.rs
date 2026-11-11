#[cfg(test)]
use crate::features::chat::channels::incoming::verify as verify_channel_signature;
#[cfg(test)]
use crate::features::chat::enums::ChannelSystemMessageAction;
use crate::features::chat::enums::ChannelSystemMessageType;
use crate::features::chat::events::ChatEvent;
use crate::features::chat::events::WS_CHANNEL_INVITE_RECEIVED;
use crate::features::chat::events::WS_CHANNELS_UPDATED;
use crate::features::chat::events::channels_updated_payload;
use crate::features::chat::events::load_key_cache;
use crate::features::chat::service::ChatService;
use crate::infrastructure::transport::contract::PeerTransport;
use plain_libs::utils::base64::base64_decode;
use plain_libs::serde_json::json;

pub fn handle<T: PeerTransport + 'static>(
    service: &ChatService<T>,
    from: &str,
    kind: ChannelSystemMessageType,
    payload: &str,
    kp_bytes: &[u8],
) -> bool {
    let result = match super::incoming::receive(
        &service.db,
        &service.identity.client_id,
        from,
        kind,
        payload,
    ) {
        Ok(result) => result,
        Err(error) => {
            log::warn!("channel message rejected: {error}");
            return false;
        }
    };
    if result.changed {
        load_key_cache(
            &service.db,
            &service.peer_key_cache,
            &service.channel_key_cache,
        );
        let _ = service.event_tx.send(ChatEvent {
            event_type: WS_CHANNELS_UPDATED,
            payload: channels_updated_payload(&service.db, &service.identity.client_id),
        });
    }
    if let Some(invite) = result.invite {
        let _=service.event_tx.send(ChatEvent {event_type:WS_CHANNEL_INVITE_RECEIVED,payload:json!({"channelId":invite.channel_id,"channelName":invite.channel_name,"fromId":invite.owner_peer_id,"fromName":invite.owner_peer_name}).to_string()});
    }
    if result.broadcast {
        if let Some(channel) = result.channel {
            let transport = service.transport.clone();
            let db = service.db.clone();
            let cache = service.peer_key_cache.clone();
            let actor = service.identity.client_id.clone();
            let name = service.identity.device_name();
            let kind = service.wire_device_type;
            let keypair = kp_bytes.to_vec();
            let key = base64_decode(&channel.key);
            tokio::spawn(async move {
                super::sender::broadcast_update(
                    &transport, &channel, &actor, &name, kind, &keypair, &db, &cache, &key,
                )
                .await;
            });
        }
    }
    result.accepted
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/chat/channels/handler.rs"]
mod tests;
