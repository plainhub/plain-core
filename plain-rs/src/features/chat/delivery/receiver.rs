use crate::features::chat::events::WS_MESSAGE_CREATED;
use crate::features::chat::manager::chat_to_json;
use crate::features::chat::service::ChatService;
use crate::infrastructure::db::DChat;
use crate::infrastructure::transport::contract::PeerTransport;
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
}
