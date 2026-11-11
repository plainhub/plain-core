use plain_libs::anyhow::Result;
use rusqlite::params;

use crate::features::chat::enums::ChatStatus;
use crate::infrastructure::db::DChat;
use crate::infrastructure::db::Db;
use crate::infrastructure::db::now_iso;

pub(crate) const CHAT_COLS: &str =
    "id,from_id,to_id,channel_id,content,status,status_data,created_at,updated_at";

pub(crate) fn row_to_chat(row: &rusqlite::Row<'_>) -> rusqlite::Result<DChat> {
    Ok(DChat {
        id: row.get(0)?,
        from_id: row.get(1)?,
        to_id: row.get(2)?,
        channel_id: row.get(3)?,
        content: row.get(4)?,
        status: row.get(5)?,
        status_data: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

impl Db {
    /// Read a `peer:<id>` or `channel:<id>` page, oldest first.
    pub fn get_chats_page(
        &self,
        target: &str,
        text: &str,
        offset: i32,
        limit: i32,
    ) -> Result<Vec<DChat>> {
        let (peer, channel) = crate::features::chat::messages::target::parse(target)?;
        let conn = self.0.lock().unwrap();
        let text = text.trim();
        let (base_sql, p1): (&str, &str) = if channel.is_empty() {
            ("WHERE channel_id IS NULL AND (to_id=?1 OR from_id=?1)", &peer)
        } else {
            ("WHERE channel_id=?1", &channel)
        };
        let text_clause = if text.is_empty() {
            String::new()
        } else {
            format!(" AND content LIKE '%{}%'", text.replace('\'', "''"))
        };
        let sql = format!(
            "SELECT {CHAT_COLS} FROM chats {base_sql}{text_clause} ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        );

        let mut stmt = conn.prepare(&sql)?;
        let mut chats = stmt
            .query_map(params![p1, limit, offset], row_to_chat)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        chats.reverse();
        Ok(chats)
    }

    pub fn insert_chat(&self, chat: &DChat) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO chats (id,from_id,to_id,channel_id,content,status,status_data,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                chat.id, chat.from_id, chat.to_id, chat.channel_id.as_deref().filter(|value|!value.is_empty()),
                chat.content, chat.status, chat.status_data.as_deref().filter(|value|!value.is_empty()),
                plain_libs::utils::dbtime::normalize(&chat.created_at)?, plain_libs::utils::dbtime::normalize(&chat.updated_at)?
            ],
        ).map(|_| ())
    }

    pub fn delete_chat(&self, id: &str) -> bool {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM chats WHERE id=?", params![id])
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    pub fn update_chat_status(&self, id: &str, status: ChatStatus) -> Option<DChat> {
        let now = now_iso();
        {
            let conn = self.0.lock().unwrap();
            let _ = conn.execute(
                "UPDATE chats SET status=?1, updated_at=?2 WHERE id=?3",
                params![status, now, id],
            );
        }
        self.get_chat_by_id(id)
    }

    pub fn update_chat_status_and_data(
        &self,
        id: &str,
        status: ChatStatus,
        status_data: &str,
    ) -> Option<DChat> {
        let now = now_iso();
        {
            let conn = self.0.lock().unwrap();
            let _ = conn.execute(
                "UPDATE chats SET status=?1, status_data=?2, updated_at=?3 WHERE id=?4",
                params![
                    status,
                    (!status_data.is_empty()).then_some(status_data),
                    now,
                    id
                ],
            );
        }
        self.get_chat_by_id(id)
    }

    pub fn get_chat_by_id(&self, id: &str) -> Option<DChat> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            &format!("SELECT {CHAT_COLS} FROM chats WHERE id=?"),
            params![id],
            row_to_chat,
        )
        .ok()
    }

    pub fn update_chat_content(&self, id: &str, content: &str) -> bool {
        let now = now_iso();
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE chats SET content=?1, updated_at=?2 WHERE id=?3",
            params![content, now, id],
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    pub fn get_all_chats(&self) -> Vec<DChat> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats ORDER BY created_at ASC"
        )) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![], row_to_chat)
            .ok()
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn delete_chats_by_channel(&self, channel_id: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("DELETE FROM chats WHERE channel_id=?", params![channel_id]);
    }

    pub fn delete_chats_by_peer(&self, peer_id: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute(
            "DELETE FROM chats WHERE channel_id IS NULL AND (to_id=? OR from_id=?)",
            params![peer_id, peer_id],
        );
    }

    pub fn delete_chats_by_ids(&self, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let conn = self.0.lock().unwrap();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM chats WHERE id IN ({placeholders})");
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let _ = conn.execute(&sql, params.as_slice());
    }

    pub fn get_chats_by_peer(&self, peer_id: &str) -> Vec<DChat> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats WHERE channel_id IS NULL AND (to_id=? OR from_id=?) ORDER BY created_at ASC"
        )) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![peer_id, peer_id], row_to_chat)
            .ok()
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    pub fn get_chats_by_channel(&self, channel_id: &str) -> Vec<DChat> {
        let conn = self.0.lock().unwrap();
        let mut stmt = match conn.prepare(&format!(
            "SELECT {CHAT_COLS} FROM chats WHERE channel_id=? ORDER BY created_at ASC"
        )) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![channel_id], row_to_chat)
            .ok()
            .map(|iter| iter.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/chat.rs"]
mod tests;
