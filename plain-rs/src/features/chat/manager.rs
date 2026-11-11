use crate::features::chat::enums::ChatStatus;
use crate::features::chat::events::ChatEvent;
use crate::features::chat::events::WS_MESSAGE_CREATED;
use crate::features::chat::events::WS_MESSAGE_DELETED;
use crate::features::chat::events::WS_MESSAGE_UPDATED;
use crate::features::chat::service::ChatService;
use crate::infrastructure::db::DChat;
use crate::infrastructure::db::Db;
use crate::infrastructure::transport::contract::PeerTransport;
use plain_libs::serde_json::{Value, json};

impl<T: PeerTransport + 'static> ChatService<T> {
    /// Send to `peer:<id>` or `channel:<id>`; `peer:local` stores a local note.
    pub fn send_chat_item(&self, to_id: String, content: String) -> Result<Vec<DChat>, String> {
        let (to, channel_id) = crate::features::chat::messages::target::parse(&to_id)
            .map_err(|error| error.to_string())?;
        let is_remote = (to != "local" && !to.is_empty()) || !channel_id.is_empty();
        let chat = crate::features::chat::messages::message_lifecycle::create(
            &self.db,
            &to,
            &channel_id,
            &content,
        )
        .map_err(|e| e.to_string())?;
        self.cacher.load(&self.db);

        self.emit(WS_MESSAGE_CREATED, json!([chat_to_json(&chat)]).to_string());

        if is_remote {
            self.spawn_delivery(&chat);
        }

        self.spawn_link_preview_refresh(&chat.id, &chat.content);

        Ok(vec![chat])
    }

    /// Delete a single chat item and broadcast `WS_MESSAGE_DELETED`.
    pub fn delete_chat_item(&self, id: String) -> bool {
        if self.db.get_chat_by_id(&id).is_none() {
            return false;
        }
        match crate::features::chat::attachments::app_file_store::chat_deletion::delete(
            &self.db,
            &self.data_dir,
            crate::features::chat::attachments::app_file_store::chat_deletion::Selection::Ids(
                std::slice::from_ref(&id),
            ),
        ) {
            Ok(0) => return false,
            Ok(_) => {}
            Err(error) => {
                log::error!("chat deletion failed: {error}");
                return false;
            }
        }
        self.cacher.load(&self.db);
        self.emit(WS_MESSAGE_DELETED, json!([id]).to_string());
        true
    }

    /// Bulk-delete chats by query (see `resolve_chat_ids`). Emits a single
    /// `WS_MESSAGE_DELETED` event whose payload is the `ids=...` string the
    /// web's `message_deleted` handler expects.
    pub fn delete_chat_items(&self, query: String) -> i32 {
        let ids = resolve_chat_ids(&self.db, &query);
        if ids.is_empty() {
            return 0;
        }
        let count = match crate::features::chat::attachments::app_file_store::chat_deletion::delete(
            &self.db,
            &self.data_dir,
            crate::features::chat::attachments::app_file_store::chat_deletion::Selection::Ids(&ids),
        ) {
            Ok(count) => count,
            Err(error) => {
                log::error!("chat deletion failed: {error}");
                return 0;
            }
        };
        self.cacher.load(&self.db);
        self.emit(WS_MESSAGE_DELETED, format!("ids={}", ids.join(",")));
        count as i32
    }

    /// Retry a failed chat item: set status to `PENDING`, emit
    /// `WS_MESSAGE_UPDATED`, then re-deliver via the same `ChatSender.send`
    /// path. The final status is computed from the actual delivery results.
    pub fn retry_chat_item(&self, id: String) -> Option<DChat> {
        let chat = self.db.get_chat_by_id(&id)?;

        // Broadcast with the updated (pending) row so the UI switches to
        // "sending" immediately — the stale object still carries FAILED.
        let updated = self.db.update_chat_status(&id, ChatStatus::Pending)?;
        self.emit(
            WS_MESSAGE_UPDATED,
            json!([chat_to_json(&updated)]).to_string(),
        );

        self.spawn_delivery(&chat);

        self.db.get_chat_by_id(&id)
    }

    /// Async link-preview refresh: rewrite the stored `content` with a
    /// `linkPreviews` array (via the app hook) and broadcast the result
    /// as `WS_MESSAGE_UPDATED`. Fire-and-forget like delivery.
    pub(crate) fn spawn_link_preview_refresh(&self, chat_id: &str, _content: &str) {
        let db = self.db.clone();
        let data_dir = self.data_dir.clone();
        let event_tx = self.event_tx.clone();
        let chat_id = chat_id.to_string();
        let link_previews = self.link_previews.clone();
        tokio::spawn(async move {
            let Some(updated) = link_previews(db.clone(), data_dir, chat_id).await else {
                return;
            };
            let _ = event_tx.send(ChatEvent {
                event_type: WS_MESSAGE_UPDATED,
                payload: json!([chat_to_json(&updated)]).to_string(),
            });
        });
    }
}

/// Build the wire JSON for a single chat item (camelCase fields). Used for
/// WS payloads and GraphQL mapping. File ids are no longer embedded —
/// clients derive them from `content` with their own urlToken.
pub fn chat_to_json(c: &DChat) -> Value {
    json!({
        "id": c.id, "fromId": c.from_id, "toId": c.to_id,
        "channelId": c.channel_id, "content": c.content,
        "createdAt": c.created_at, "updatedAt": c.updated_at,
        "status": c.status, "statusData": c.status_data.as_deref().unwrap_or_default(),
    })
}

/// Resolve a `deleteChatItems(query)` query into the list of chat ids that
/// should be removed. Mirrors plain-app's `ChatDbHelper.getIdsAsync(query)`:
///   * `ids:<comma-separated-ids>`   — return the listed ids verbatim.
///   * `channel:<channelId>`         — every chat id in the channel.
///   * `peer:<peerId>`               — every 1:1 chat id with the peer.
///   * `peer:local`                  — every local-note chat id.
///
/// Returns an empty Vec for an unrecognized / empty query.
pub fn resolve_chat_ids(db: &Db, query: &str) -> Vec<String> {
    let query = query.trim();
    if query.is_empty() {
        return vec![];
    }
    let Some((name, value)) = query.split_once(':') else {
        return vec![];
    };
    match name {
        "ids" => value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        "channel" => db
            .get_chats_by_channel(value)
            .into_iter()
            .map(|c| c.id)
            .collect(),
        "peer" => {
            let peer_id = if value == "local" { "local" } else { value };
            db.get_chats_by_peer(peer_id)
                .into_iter()
                .map(|c| c.id)
                .collect()
        }
        _ => vec![],
    }
}
