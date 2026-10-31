use rusqlite::types::Value;
use rusqlite::{OptionalExtension, params, params_from_iter};

use crate::db::Db;
use crate::db::models::simple::ArchivedConversationRow;
use crate::db::models::simple::ImageEmbeddingRow;
use crate::db::models::simple::PomodoroItemRow;
use crate::db::models::simple::SessionRow;
use crate::db::models::simple::ShareRow;
use crate::db::models::simple::TrashedMessageRow;

const SESSION_COLUMNS: &str = "client_id, name, type, client_ip, os_name, os_version, \
     browser_name, browser_version, token, url_token, last_seen_at, created_at, updated_at";
const SHARE_COLUMNS: &str = "id, name, password, url_token, expires_at, read_only, data, \
     created_at, updated_at";
const POMODORO_COLUMNS: &str =
    "id, date, completed_count, total_work_secs, total_break_secs, created_at, updated_at";
const EMBEDDING_COLUMNS: &str = "file_id, path, embedding, created_at, updated_at";

fn session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        client_id: row.get(0)?,
        name: row.get(1)?,
        r#type: row.get(2)?,
        client_ip: row.get(3)?,
        os_name: row.get(4)?,
        os_version: row.get(5)?,
        browser_name: row.get(6)?,
        browser_version: row.get(7)?,
        token: row.get(8)?,
        url_token: row.get(9)?,
        last_seen_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn share_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShareRow> {
    Ok(ShareRow {
        id: row.get(0)?,
        name: row.get(1)?,
        password: row.get(2)?,
        url_token: row.get(3)?,
        expires_at: row.get(4)?,
        read_only: row.get::<_, i64>(5)? != 0,
        data: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn pomodoro_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PomodoroItemRow> {
    Ok(PomodoroItemRow {
        id: row.get(0)?,
        date: row.get(1)?,
        completed_count: row.get(2)?,
        total_work_secs: row.get(3)?,
        total_break_secs: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn embedding_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImageEmbeddingRow> {
    let blob: Vec<u8> = row.get(2)?;
    Ok(ImageEmbeddingRow {
        file_id: row.get(0)?,
        path: row.get(1)?,
        embedding_base64: crate::utils::base64::base64_encode(&blob),
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn in_clause(column: &str, values: &[String]) -> (String, Vec<Value>) {
    if values.is_empty() {
        return ("0=1".to_string(), Vec::new());
    }
    let placeholders = vec!["?"; values.len()].join(",");
    (
        format!("{column} IN ({placeholders})"),
        values.iter().map(|v| Value::Text(v.clone())).collect(),
    )
}

impl Db {
    pub fn session_list(&self) -> rusqlite::Result<Vec<SessionRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {SESSION_COLUMNS} FROM sessions ORDER BY last_seen_at DESC"
            ))?;
            stmt.query_map([], session_from_row)?.collect()
        })
    }

    pub fn session_get(&self, client_id: &str) -> rusqlite::Result<Option<SessionRow>> {
        self.with_conn(|c| {
            c.query_row(
                &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE client_id=?1"),
                [client_id],
                session_from_row,
            )
            .optional()
        })
    }

    pub fn session_save(&self, row: &SessionRow) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO sessions (client_id, name, type, client_ip, os_name,
                     os_version, browser_name, browser_version, token, url_token, last_seen_at,
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    row.client_id,
                    row.name,
                    row.r#type,
                    row.client_ip,
                    row.os_name,
                    row.os_version,
                    row.browser_name,
                    row.browser_version,
                    row.token,
                    row.url_token,
                    row.last_seen_at
                        .as_deref()
                        .map(crate::utils::dbtime::normalize)
                        .transpose()?,
                    crate::utils::dbtime::normalize(&row.created_at)?,
                    crate::utils::dbtime::normalize(&row.updated_at)?
                ],
            )?;
            Ok(())
        })
    }

    /// Bumps only `last_seen_at` for the listed clients (the `updateTs` DAO).
    pub fn session_touch(&self, items: &[(String, String)]) -> rusqlite::Result<usize> {
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let mut updated = 0;
            {
                let mut stmt =
                    tx.prepare("UPDATE sessions SET last_seen_at=?2 WHERE client_id=?1")?;
                for (client_id, at) in items {
                    updated +=
                        stmt.execute(params![client_id, crate::utils::dbtime::normalize(at)?])?;
                }
            }
            tx.commit()?;
            Ok(updated)
        })
    }

    pub fn session_rename(
        &self,
        client_id: &str,
        name: &str,
        updated_at: &str,
    ) -> rusqlite::Result<usize> {
        self.with_conn(|connection| {
            connection.execute(
                "UPDATE sessions SET name=?2,updated_at=?3 WHERE client_id=?1",
                rusqlite::params![client_id, name, updated_at],
            )
        })
    }

    pub fn session_delete(&self, client_id: &str) -> rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM sessions WHERE client_id=?1", [client_id]))
    }

    pub fn share_list(&self) -> rusqlite::Result<Vec<ShareRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {SHARE_COLUMNS} FROM shares ORDER BY created_at DESC"
            ))?;
            stmt.query_map([], share_from_row)?.collect()
        })
    }

    pub fn share_get(&self, id: &str) -> rusqlite::Result<Option<ShareRow>> {
        self.with_conn(|c| {
            c.query_row(
                &format!("SELECT {SHARE_COLUMNS} FROM shares WHERE id=?1"),
                [id],
                share_from_row,
            )
            .optional()
        })
    }

    pub fn share_save(&self, row: &ShareRow) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO shares (id, name, password, url_token, expires_at,
                     read_only, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    row.id,
                    row.name,
                    row.password,
                    row.url_token,
                    row.expires_at
                        .as_deref()
                        .map(crate::utils::dbtime::normalize)
                        .transpose()?,
                    row.read_only as i64,
                    row.data,
                    crate::utils::dbtime::normalize(&row.created_at)?,
                    crate::utils::dbtime::normalize(&row.updated_at)?
                ],
            )?;
            Ok(())
        })
    }

    pub fn share_update(
        &self,
        id: &str,
        name: &str,
        expires_at: Option<&str>,
        data: Option<&str>,
        now: &str,
    ) -> rusqlite::Result<bool> {
        self.with_conn(|c| Ok(c.execute("UPDATE shares SET name=?2, expires_at=?3, data=COALESCE(?4,data), updated_at=?5 WHERE id=?1", params![id,name,expires_at,data,now])? != 0))
    }

    pub fn share_delete(&self, id: &str) -> rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM shares WHERE id=?1", [id]))
    }

    pub fn pomodoro_list(&self) -> rusqlite::Result<Vec<PomodoroItemRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {POMODORO_COLUMNS} FROM pomodoro_items ORDER BY date DESC"
            ))?;
            stmt.query_map([], pomodoro_from_row)?.collect()
        })
    }

    pub fn pomodoro_get_by_date(&self, date: &str) -> rusqlite::Result<Option<PomodoroItemRow>> {
        self.with_conn(|c| {
            c.query_row(
                &format!("SELECT {POMODORO_COLUMNS} FROM pomodoro_items WHERE date=?1"),
                [date],
                pomodoro_from_row,
            )
            .optional()
        })
    }

    pub fn pomodoro_recent(
        &self,
        start_date: &str,
        limit: i64,
    ) -> rusqlite::Result<Vec<PomodoroItemRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {POMODORO_COLUMNS} FROM pomodoro_items WHERE date >= ?1
                 ORDER BY date DESC LIMIT ?2"
            ))?;
            stmt.query_map(params![start_date, limit.max(0)], pomodoro_from_row)?
                .collect()
        })
    }

    pub fn pomodoro_total_completed(&self) -> rusqlite::Result<i64> {
        self.with_conn(|c| {
            c.query_row("SELECT SUM(completed_count) FROM pomodoro_items", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .map(|v| v.unwrap_or(0))
        })
    }

    pub fn pomodoro_save(&self, row: &PomodoroItemRow) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO pomodoro_items (id, date, completed_count,
                     total_work_secs, total_break_secs, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row.id,
                    row.date,
                    row.completed_count,
                    row.total_work_secs,
                    row.total_break_secs,
                    crate::utils::dbtime::normalize(&row.created_at)?,
                    crate::utils::dbtime::normalize(&row.updated_at)?
                ],
            )?;
            Ok(())
        })
    }

    pub fn pomodoro_delete(&self, id: &str) -> rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM pomodoro_items WHERE id=?1", [id]))
    }

    pub fn embedding_list(&self) -> rusqlite::Result<Vec<ImageEmbeddingRow>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare(&format!("SELECT {EMBEDDING_COLUMNS} FROM image_embeddings"))?;
            stmt.query_map([], embedding_from_row)?.collect()
        })
    }

    pub fn embedding_ids(&self) -> rusqlite::Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT file_id FROM image_embeddings")?;
            stmt.query_map([], |r| r.get(0))?.collect()
        })
    }

    pub fn embedding_count(&self) -> rusqlite::Result<i64> {
        self.with_conn(|c| c.query_row("SELECT COUNT(*) FROM image_embeddings", [], |r| r.get(0)))
    }

    pub fn embedding_save(&self, row: &ImageEmbeddingRow) -> rusqlite::Result<()> {
        let blob = crate::utils::base64::base64_decode_checked(&row.embedding_base64)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        crate::utils::vector::decode_f32_be(&blob)
            .map_err(rusqlite::Error::InvalidParameterName)?;
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO image_embeddings (file_id, path, embedding, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row.file_id, row.path, blob, crate::utils::dbtime::normalize(&row.created_at)?, crate::utils::dbtime::normalize(&row.updated_at)?],
            )?;
            Ok(())
        })
    }

    pub fn embedding_delete_all(&self) -> rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM image_embeddings", []))
    }

    pub fn embedding_delete_by_ids(&self, ids: &[String]) -> rusqlite::Result<usize> {
        let (clause, values) = in_clause("file_id", ids);
        self.with_conn(|c| {
            c.execute(
                &format!("DELETE FROM image_embeddings WHERE {clause}"),
                params_from_iter(values),
            )
        })
    }

    pub fn archived_conversation_list(&self) -> rusqlite::Result<Vec<ArchivedConversationRow>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare("SELECT conversation_id, archived_at FROM archived_sms_conversations")?;
            stmt.query_map([], |r| {
                Ok(ArchivedConversationRow {
                    conversation_id: r.get(0)?,
                    archived_at: r.get(1)?,
                })
            })?
            .collect()
        })
    }

    pub fn archived_conversation_save(
        &self,
        row: &ArchivedConversationRow,
    ) -> rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO archived_sms_conversations (conversation_id, archived_at)
                 VALUES (?1, ?2)",
                params![
                    row.conversation_id,
                    crate::utils::dbtime::normalize(&row.archived_at)?
                ],
            )?;
            Ok(())
        })
    }

    pub fn archived_conversation_delete(&self, conversation_id: &str) -> rusqlite::Result<usize> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM archived_sms_conversations WHERE conversation_id=?1",
                [conversation_id],
            )
        })
    }

    pub fn trashed_message_ids(&self) -> rusqlite::Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT CASE WHEN is_mms=1 THEN 'mms_' || message_id ELSE message_id END FROM trashed_sms")?;
            stmt.query_map([], |r| r.get(0))?.collect()
        })
    }

    pub fn trashed_message_save_many(&self, rows: &[TrashedMessageRow]) -> rusqlite::Result<usize> {
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let mut inserted = 0;
            {
                let mut stmt = tx.prepare(
                    "INSERT OR IGNORE INTO trashed_sms (message_id, is_mms, trashed_at)
                     VALUES (?1, ?2, ?3)",
                )?;
                for row in rows {
                    inserted += stmt.execute(params![
                        row.message_id,
                        row.is_mms as i64,
                        crate::utils::dbtime::normalize(&row.trashed_at)?
                    ])?;
                }
            }
            tx.commit()?;
            Ok(inserted)
        })
    }

    pub fn trashed_message_delete_by_ids(&self, ids: &[String]) -> rusqlite::Result<usize> {
        self.with_conn(|c| {
            let tx = c.unchecked_transaction()?;
            let mut count = 0;
            for id in ids {
                let (message_id, is_mms) = match id.strip_prefix("mms_") {
                    Some(id) => (id, true),
                    None => (id.as_str(), false),
                };
                count += tx.execute(
                    "DELETE FROM trashed_sms WHERE message_id=?1 AND is_mms=?2",
                    params![message_id, is_mms],
                )?;
            }
            tx.commit()?;
            Ok(count)
        })
    }

    pub fn trashed_message_delete_older_than(&self, before: &str) -> rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM trashed_sms WHERE trashed_at < ?1", [before]))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/simple.rs"]
mod simple_tests;
