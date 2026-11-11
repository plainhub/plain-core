use crate::features::chat::channels::handler as channel_handler;
use crate::features::chat::enums::ChannelSystemMessageType;
use crate::features::chat::events::WS_CHANNELS_UPDATED;
use crate::features::chat::events::WS_MESSAGE_CREATED;
use crate::features::chat::events::channels_updated_payload;
use crate::features::chat::manager::chat_to_json;
use crate::features::chat::service::ChatService;
use crate::infrastructure::db::DChat;
use crate::infrastructure::transport::contract::PeerTransport;
use plain_libs::utils::base64::base64_decode;
use plain_libs::serde_json::json;

impl<T: PeerTransport + 'static> ChatService<T> {
    /// Persist an incoming chat item from a peer and broadcast
    /// `WS_MESSAGE_CREATED`. Returns the persisted row (or a synthetic,
    /// un-persisted row when the channel gate drops the message) for the
    /// GraphQL response.
    pub fn receive_peer_chat(
        &self,
        from_id: &str,
        channel_id: &str,
        content: &str,
        signature: &str,
        timestamp: i64,
    ) -> Result<Option<DChat>, String> {
        let Some(received) = crate::features::chat::messages::message_lifecycle::receive(
            &self.db, from_id, channel_id, content, signature, timestamp,
        )
        .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let chat = received.chat;
        self.cacher.load(&self.db);
        self.emit(WS_MESSAGE_CREATED, json!([chat_to_json(&chat)]).to_string());

        self.spawn_link_preview_refresh(&chat.id, &chat.content);
        Ok(Some(chat))
    }

    /// Dispatch an incoming `channelSystemMessage` to the local channel handler
    /// and broadcast `WS_CHANNELS_UPDATED` so local UI can refresh. Returns the
    /// boolean the peer expects from the GraphQL contract.
    pub fn receive_peer_channel_system_message(
        &self,
        from_id: &str,
        msg_type: ChannelSystemMessageType,
        payload: &str,
    ) -> bool {
        let kp_bytes = base64_decode(&self.identity.ed25519_keypair);
        let ok = channel_handler::handle(self, from_id, msg_type, payload, &kp_bytes);
        self.emit(
            WS_CHANNELS_UPDATED,
            channels_updated_payload(&self.db, &self.identity.client_id),
        );
        ok
    }
}
