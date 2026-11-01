use crate::features::chat::events::ChatEvent;
use crate::features::chat::events::WS_MESSAGE_UPDATED;
use crate::features::chat::manager::chat_to_json;
use crate::features::chat::service::ChatService;
use crate::infrastructure::db::DChat;
use crate::infrastructure::transport::contract::PeerTransport;
use serde_json::json;

impl<T: PeerTransport + 'static> ChatService<T> {
    pub(crate) fn spawn_delivery(&self, chat: &DChat) {
        if chat.to_id == "local" || (chat.to_id.is_empty() && chat.channel_id.is_none()) {
            return;
        }
        let delivery = self.delivery.clone();
        let transport = self.transport.clone();
        let identity = self.identity.clone();
        let token = self.token.clone();
        let id = chat.id.clone();
        let events = self.event_tx.clone();
        let hooks = self.hooks.clone();
        tokio::spawn(async move {
            match delivery
                .send(
                    &transport,
                    &identity.client_id,
                    &plain_libs::utils::base64::base64_decode(&identity.ed25519_keypair),
                    &token,
                    &id,
                    None,
                )
                .await
            {
                Ok(receipt) => {
                    if receipt.rediscover {
                        hooks.rebrowse_peers();
                    }
                    if let Some(updated) = receipt.chat {
                        let _ = events.send(ChatEvent {
                            event_type: WS_MESSAGE_UPDATED,
                            payload: json!([chat_to_json(&updated)]).to_string(),
                        });
                    }
                }
                Err(error) => log::error!("chat delivery failed: {error}"),
            }
        });
    }
}
