use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::infrastructure::db::DChat;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::chat_store::channels;
use crate::infrastructure::db::chat_store::messages;
use crate::infrastructure::db::chat_store::peers;
use plain_libs::anyhow::Result;

pub struct ChatCacher {
    latest_chat_map: RwLock<HashMap<String, DChat>>,
}

impl ChatCacher {
    pub fn new() -> Self {
        Self {
            latest_chat_map: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_latest_chat(&self, chat_id: &str) -> Option<DChat> {
        self.latest_chat_map.read().unwrap().get(chat_id).cloned()
    }

    pub fn snapshot(db: &Db) -> Result<HashMap<String, DChat>> {
        Ok(Self::conversations(db)?.into_iter().collect())
    }

    pub fn conversations(db: &Db) -> Result<Vec<(String, DChat)>> {
        let peer_ids: HashSet<String> = peers::all(db)?.into_iter().map(|p| p.id).collect();
        let channel_ids: HashSet<String> = channels::all(db)?.into_iter().map(|c| c.id).collect();
        let latest_chats: Vec<DChat> = serde_json::from_value(messages::list(
            db,
            &messages::Filter {
                peer: None,
                channel: None,
                text: String::new(),
                offset: 0,
                limit: None,
                descending: true,
                latest: true,
                count_only: false,
            },
        )?)?;
        let mut chat_cache = HashMap::<String, DChat>::new();
        for chat in latest_chats {
            let chat_id = if let Some(id) = chat.channel_id.as_ref().filter(|id| !id.is_empty()) {
                channel_ids.contains(id).then(|| id.clone())
            } else if (chat.from_id == "me" && chat.to_id == "local")
                || (chat.from_id == "local" && chat.to_id == "me")
            {
                Some("local".to_string())
            } else if chat.from_id == "me" && peer_ids.contains(&chat.to_id) {
                Some(chat.to_id.clone())
            } else if chat.to_id == "me" && peer_ids.contains(&chat.from_id) {
                Some(chat.from_id.clone())
            } else {
                None
            };
            if let Some(chat_id) = chat_id {
                let created_at = plain_libs::chrono::DateTime::parse_from_rfc3339(&chat.created_at)?;
                let should_replace = match chat_cache.get(&chat_id) {
                    None => true,
                    Some(existing) => {
                        (created_at, &chat.id)
                            > (
                                plain_libs::chrono::DateTime::parse_from_rfc3339(&existing.created_at)?,
                                &existing.id,
                            )
                    }
                };
                if should_replace {
                    chat_cache.insert(chat_id, chat);
                }
            }
        }
        let mut rows = chat_cache
            .into_iter()
            .map(|(id, chat)| {
                Ok((
                    plain_libs::chrono::DateTime::parse_from_rfc3339(&chat.created_at)?,
                    id,
                    chat,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        rows.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.2.id.cmp(&left.2.id))
                .then_with(|| left.1.cmp(&right.1))
        });
        Ok(rows.into_iter().map(|(_, id, chat)| (id, chat)).collect())
    }

    pub fn load(&self, db: &Db) {
        match Self::snapshot(db) {
            Ok(snapshot) => *self.latest_chat_map.write().unwrap() = snapshot,
            Err(error) => log::error!("[chat] latest conversation snapshot failed: {error}"),
        }
    }
}

impl Default for ChatCacher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/chat/cacher.rs"]
mod tests;
