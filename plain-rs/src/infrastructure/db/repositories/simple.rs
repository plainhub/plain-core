use plain_libs::rusqlite::types::Value;
use plain_libs::rusqlite::{OptionalExtension, params, params_from_iter};

use crate::infrastructure::db::Db;
use crate::infrastructure::db::models::simple::ArchivedConversationRow;
use crate::infrastructure::db::models::simple::ImageEmbeddingRow;
use crate::infrastructure::db::models::simple::PomodoroItemRow;
use crate::infrastructure::db::models::simple::SessionRow;
use crate::infrastructure::db::models::simple::ShareRow;
use crate::infrastructure::db::models::simple::TrashedMessageRow;

const SESSION_COLUMNS: &str = "client_id, name, type, client_ip, os_name, os_version, \
     app_name, app_version, token, url_token, last_seen_at, created_at, updated_at";
const SHARE_COLUMNS: &str = "id, name, password, url_token, expires_at, read_only, data, \
     created_at, updated_at";
const POMODORO_COLUMNS: &str =
    "id, date, completed_count, total_work_secs, total_break_secs, created_at, updated_at";
const EMBEDDING_COLUMNS: &str = "file_id, path, embedding, created_at, updated_at";

fn session_from_row(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        client_id: row.get(0)?,
        name: row.get(1)?,
        r#type: row.get(2)?,
        client_ip: row.get(3)?,
        os_name: row.get(4)?,
        os_version: row.get(5)?,
        app_name: row.get(6)?,
        app_version: row.get(7)?,
        token: row.get(8)?,
        url_token: row.get(9)?,
        last_seen_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn share_from_row(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<ShareRow> {
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

fn pomodoro_from_row(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<PomodoroItemRow> {
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

fn embedding_from_row(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<ImageEmbeddingRow> {
    let blob: Vec<u8> = row.get(2)?;
    Ok(ImageEmbeddingRow {
        file_id: row.get(0)?,
        path: row.get(1)?,
        embedding_base64: plain_libs::utils::base64::base64_encode(&blob),
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

#[cfg(feature = "chat")]
pub(crate) fn session_snapshot(
    connection: &plain_libs::rusqlite::Connection,
) -> plain_libs::rusqlite::Result<std::collections::HashMap<String, SessionRow>> {
    let mut statement = connection.prepare(&format!("SELECT {SESSION_COLUMNS} FROM sessions"))?;
    statement
        .query_map([], session_from_row)?
        .map(|row| row.map(|row| (row.client_id.clone(), row)))
        .collect()
}

impl Db {
    pub fn session_list(&self) -> plain_libs::rusqlite::Result<Vec<SessionRow>> {
        #[cfg(feature = "chat")]
        {
            if self.cached_sessions().is_none() {
                self.with_conn(|connection| self.reload_sessions(connection))?;
            }
            let mut rows = self
                .1
                .sessions
                .read()
                .unwrap()
                .rows
                .values()
                .cloned()
                .collect::<Vec<_>>();
            rows.sort_by(|a, b| {
                b.last_seen_at
                    .cmp(&a.last_seen_at)
                    .then_with(|| a.client_id.cmp(&b.client_id))
            });
            return Ok(rows);
        }
        #[cfg(not(feature = "chat"))]
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {SESSION_COLUMNS} FROM sessions ORDER BY last_seen_at DESC"
            ))?;
            stmt.query_map([], session_from_row)?.collect()
        })
    }

    pub fn session_get(&self, client_id: &str) -> plain_libs::rusqlite::Result<Option<SessionRow>> {
        #[cfg(feature = "chat")]
        {
            if let Some(row) = self.cached_session(client_id) {
                return Ok(row);
            }
            self.with_conn(|connection| {
                if let Some(row) = self.cached_session(client_id) {
                    return Ok(row);
                }
                self.reload_sessions(connection)?;
                let snapshot = self.1.sessions.read().unwrap();
                Ok(snapshot.rows.get(client_id).cloned())
            })
        }
        #[cfg(not(feature = "chat"))]
        self.with_conn(|connection| {
            connection
                .query_row(
                    &format!("SELECT {SESSION_COLUMNS} FROM sessions WHERE client_id=?1"),
                    [client_id],
                    session_from_row,
                )
                .optional()
        })
    }

    #[cfg(feature = "chat")]
    fn cached_session(&self, client_id: &str) -> Option<Option<SessionRow>> {
        self.cached_sessions()
            .map(|snapshot| snapshot.rows.get(client_id).cloned())
    }

    #[cfg(feature = "chat")]
    fn cached_sessions(
        &self,
    ) -> Option<std::sync::RwLockReadGuard<'_, super::super::state::SessionSnapshot>> {
        let snapshot = self.1.sessions.read().unwrap();
        (snapshot.activity_flushing
            || snapshot.revision
                == self
                    .1
                    .session_revision
                    .load(std::sync::atomic::Ordering::Acquire))
        .then_some(snapshot)
    }

    #[cfg(feature = "chat")]
    fn reload_sessions(&self, connection: &plain_libs::rusqlite::Connection) -> plain_libs::rusqlite::Result<()> {
        let rows = session_snapshot(connection)?;
        let mut snapshot = self.1.sessions.write().unwrap();
        let old = std::mem::replace(&mut snapshot.rows, rows);
        let super::super::state::SessionSnapshot { rows, activity, .. } = &mut *snapshot;
        activity.retain(|(id, at)| {
            old.get(id).zip(rows.get(id)).is_some_and(|(old, new)| {
                old.token == new.token
                    && old.url_token == new.url_token
                    && new
                        .last_seen_at
                        .as_deref()
                        .is_none_or(|persisted| persisted < at.as_str())
            })
        });
        for (id, at) in activity.pending() {
            if let Some(row) = rows.get_mut(id) {
                row.last_seen_at = Some(at.clone());
            }
        }
        snapshot.revision = self
            .1
            .session_revision
            .load(std::sync::atomic::Ordering::Acquire);
        Ok(())
    }

    #[cfg(feature = "chat")]
    fn update_session_snapshot(
        &self,
        before: u64,
        update: impl FnOnce(&mut super::super::state::SessionSnapshot),
    ) {
        let mut snapshot = self.1.sessions.write().unwrap();
        let current = snapshot.revision == before;
        update(&mut snapshot);
        if current {
            snapshot.revision = self
                .1
                .session_revision
                .load(std::sync::atomic::Ordering::Acquire);
        }
    }

    pub fn session_save(&self, row: &SessionRow) -> plain_libs::rusqlite::Result<()> {
        let mut row = row.clone();
        row.created_at = plain_libs::utils::dbtime::normalize(&row.created_at)?;
        row.updated_at = plain_libs::utils::dbtime::normalize(&row.updated_at)?;
        row.last_seen_at = row
            .last_seen_at
            .as_deref()
            .map(plain_libs::utils::dbtime::normalize)
            .transpose()?;
        self.with_conn(|connection| {
            #[cfg(feature = "chat")]
            let before = self.1.session_revision.load(std::sync::atomic::Ordering::Acquire);
            connection.execute("INSERT OR REPLACE INTO sessions (client_id,name,type,client_ip,os_name,os_version,app_name,app_version,token,url_token,last_seen_at,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![row.client_id,row.name,row.r#type,row.client_ip,row.os_name,row.os_version,row.app_name,row.app_version,row.token,row.url_token,row.last_seen_at,row.created_at,row.updated_at])?;
            #[cfg(feature = "chat")]
            self.update_session_snapshot(before, |snapshot| {snapshot.activity.retain(|(id,_)| id != &row.client_id);snapshot.rows.insert(row.client_id.clone(),row);});
            Ok(())
        })
    }

    pub fn session_touch(&self, items: &[(String, String)]) -> plain_libs::rusqlite::Result<usize> {
        let items = items
            .iter()
            .map(|(id, at)| Ok((id, plain_libs::utils::dbtime::normalize(at)?)))
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
        #[cfg(feature = "chat")]
        {
            for (id, _) in &items {
                self.session_get(id)?;
            }
            let mut snapshot = self.1.sessions.write().unwrap();
            let now = std::time::Instant::now();
            let mut updated = 0;
            for (id, at) in items {
                if let Some(row) = snapshot.rows.get_mut(id) {
                    row.last_seen_at = Some(at.clone());
                    snapshot.activity.push((id.clone(), at), now);
                    updated += 1;
                }
            }
            Ok(updated)
        }
        #[cfg(not(feature = "chat"))]
        self.with_conn(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut updated = 0;
            for (id, at) in items {
                updated += transaction.execute(
                    "UPDATE sessions SET last_seen_at=?2 WHERE client_id=?1",
                    params![id, at],
                )?;
            }
            transaction.commit()?;
            Ok(updated)
        })
    }

    #[cfg(feature = "chat")]
    pub fn flush_session_activity(
        &self,
        now: std::time::Instant,
        force: bool,
    ) -> plain_libs::rusqlite::Result<bool> {
        {
            let snapshot = self.1.sessions.read().unwrap();
            if snapshot.activity.pending().is_empty() || (!force && !snapshot.activity.is_due(now))
            {
                return Ok(false);
            }
        }
        self.with_conn(|connection| {
            if self.cached_sessions().is_none() {
                self.reload_sessions(connection)?;
            }
            let mut batch = {
                let mut snapshot = self.1.sessions.write().unwrap();
                snapshot.activity_flushing = true;
                snapshot.activity.take_pending()
            };
            let write = |pending: &[(String, String)]| -> plain_libs::rusqlite::Result<()> {
                let latest = pending
                    .iter()
                    .map(|(id, at)| (id, at))
                    .collect::<std::collections::BTreeMap<_, _>>();
                let transaction = connection.unchecked_transaction()?;
                for (id, at) in latest {
                    transaction.execute(
                        "UPDATE sessions SET last_seen_at=?2 WHERE client_id=?1",
                        params![id, at],
                    )?;
                }
                transaction.commit()
            };
            let result = if force {
                batch.flush(write)
            } else {
                batch.flush_due(now, write)
            };
            self.1
                .sessions
                .write()
                .unwrap()
                .activity
                .restore_pending(batch);
            let reloaded = self.reload_sessions(connection);
            self.1.sessions.write().unwrap().activity_flushing = false;
            reloaded?;
            result
        })
    }

    pub fn session_rename(
        &self,
        client_id: &str,
        name: &str,
        updated_at: &str,
    ) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|connection| {
            #[cfg(feature = "chat")]
            let before = self
                .1
                .session_revision
                .load(std::sync::atomic::Ordering::Acquire);
            let updated = connection.execute(
                "UPDATE sessions SET name=?2,updated_at=?3 WHERE client_id=?1",
                params![client_id, name, updated_at],
            )?;
            #[cfg(feature = "chat")]
            self.update_session_snapshot(before, |snapshot| {
                if let Some(row) = snapshot.rows.get_mut(client_id) {
                    row.name = name.into();
                    row.updated_at = updated_at.into();
                }
            });
            Ok(updated)
        })
    }

    pub fn session_delete(&self, client_id: &str) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|connection| {
            #[cfg(feature = "chat")]
            let before = self
                .1
                .session_revision
                .load(std::sync::atomic::Ordering::Acquire);
            let updated =
                connection.execute("DELETE FROM sessions WHERE client_id=?1", [client_id])?;
            #[cfg(feature = "chat")]
            self.update_session_snapshot(before, |snapshot| {
                snapshot.activity.retain(|(id, _)| id != client_id);
                snapshot.rows.remove(client_id);
            });
            Ok(updated)
        })
    }

    pub fn share_list(&self) -> plain_libs::rusqlite::Result<Vec<ShareRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {SHARE_COLUMNS} FROM shares ORDER BY created_at DESC"
            ))?;
            stmt.query_map([], share_from_row)?.collect()
        })
    }

    pub fn share_get(&self, id: &str) -> plain_libs::rusqlite::Result<Option<ShareRow>> {
        self.with_conn(|c| {
            c.query_row(
                &format!("SELECT {SHARE_COLUMNS} FROM shares WHERE id=?1"),
                [id],
                share_from_row,
            )
            .optional()
        })
    }

    pub fn share_insert(&self, row: &ShareRow) -> plain_libs::rusqlite::Result<bool> {
        self.with_conn(|c| {
            let inserted = c.execute(
                "INSERT INTO shares (id, name, password, url_token, expires_at,
                     read_only, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO NOTHING",
                params![
                    row.id,
                    row.name,
                    row.password,
                    row.url_token,
                    row.expires_at
                        .as_deref()
                        .map(plain_libs::utils::dbtime::normalize)
                        .transpose()?,
                    row.read_only as i64,
                    row.data,
                    plain_libs::utils::dbtime::normalize(&row.created_at)?,
                    plain_libs::utils::dbtime::normalize(&row.updated_at)?
                ],
            )?;
            Ok(inserted != 0)
        })
    }

    pub fn share_update(
        &self,
        id: &str,
        name: &str,
        expires_at: Option<&str>,
        data: Option<&str>,
        now: &str,
    ) -> plain_libs::rusqlite::Result<bool> {
        self.with_conn(|c| Ok(c.execute("UPDATE shares SET name=?2, expires_at=?3, data=COALESCE(?4,data), updated_at=?5 WHERE id=?1", params![id,name,expires_at,data,now])? != 0))
    }

    pub fn share_delete(&self, id: &str) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM shares WHERE id=?1", [id]))
    }

    pub fn pomodoro_list(&self) -> plain_libs::rusqlite::Result<Vec<PomodoroItemRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {POMODORO_COLUMNS} FROM pomodoro_items ORDER BY date DESC"
            ))?;
            stmt.query_map([], pomodoro_from_row)?.collect()
        })
    }

    pub fn pomodoro_get_by_date(&self, date: &str) -> plain_libs::rusqlite::Result<Option<PomodoroItemRow>> {
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
    ) -> plain_libs::rusqlite::Result<Vec<PomodoroItemRow>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {POMODORO_COLUMNS} FROM pomodoro_items WHERE date >= ?1
                 ORDER BY date DESC LIMIT ?2"
            ))?;
            stmt.query_map(params![start_date, limit.max(0)], pomodoro_from_row)?
                .collect()
        })
    }

    pub fn pomodoro_total_completed(&self) -> plain_libs::rusqlite::Result<i64> {
        self.with_conn(|c| {
            c.query_row("SELECT SUM(completed_count) FROM pomodoro_items", [], |r| {
                r.get::<_, Option<i64>>(0)
            })
            .map(|v| v.unwrap_or(0))
        })
    }

    pub fn pomodoro_save(&self, row: &PomodoroItemRow) -> plain_libs::rusqlite::Result<()> {
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
                    plain_libs::utils::dbtime::normalize(&row.created_at)?,
                    plain_libs::utils::dbtime::normalize(&row.updated_at)?
                ],
            )?;
            Ok(())
        })
    }

    pub fn pomodoro_delete(&self, id: &str) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM pomodoro_items WHERE id=?1", [id]))
    }

    pub fn embedding_list(&self) -> plain_libs::rusqlite::Result<Vec<ImageEmbeddingRow>> {
        self.with_conn(|c| {
            let mut stmt =
                c.prepare(&format!("SELECT {EMBEDDING_COLUMNS} FROM image_embeddings"))?;
            stmt.query_map([], embedding_from_row)?.collect()
        })
    }

    pub fn embedding_ids(&self) -> plain_libs::rusqlite::Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT file_id FROM image_embeddings")?;
            stmt.query_map([], |r| r.get(0))?.collect()
        })
    }

    pub fn embedding_count(&self) -> plain_libs::rusqlite::Result<i64> {
        self.with_conn(|c| c.query_row("SELECT COUNT(*) FROM image_embeddings", [], |r| r.get(0)))
    }

    pub fn embedding_save(&self, row: &ImageEmbeddingRow) -> plain_libs::rusqlite::Result<()> {
        let blob = plain_libs::utils::base64::base64_decode_checked(&row.embedding_base64)
            .map_err(|e| plain_libs::rusqlite::Error::InvalidParameterName(e.to_string()))?;
        plain_libs::utils::vector::decode_f32_be(&blob)
            .map_err(plain_libs::rusqlite::Error::InvalidParameterName)?;
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO image_embeddings (file_id, path, embedding, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row.file_id, row.path, blob, plain_libs::utils::dbtime::normalize(&row.created_at)?, plain_libs::utils::dbtime::normalize(&row.updated_at)?],
            )?;
            Ok(())
        })
    }

    pub fn embedding_delete_all(&self) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM image_embeddings", []))
    }

    pub fn embedding_delete_by_ids(&self, ids: &[String]) -> plain_libs::rusqlite::Result<usize> {
        let (clause, values) = in_clause("file_id", ids);
        self.with_conn(|c| {
            c.execute(
                &format!("DELETE FROM image_embeddings WHERE {clause}"),
                params_from_iter(values),
            )
        })
    }

    pub fn archived_conversation_list(&self) -> plain_libs::rusqlite::Result<Vec<ArchivedConversationRow>> {
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
    ) -> plain_libs::rusqlite::Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT OR REPLACE INTO archived_sms_conversations (conversation_id, archived_at)
                 VALUES (?1, ?2)",
                params![
                    row.conversation_id,
                    plain_libs::utils::dbtime::normalize(&row.archived_at)?
                ],
            )?;
            Ok(())
        })
    }

    pub fn archived_conversation_delete(&self, conversation_id: &str) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM archived_sms_conversations WHERE conversation_id=?1",
                [conversation_id],
            )
        })
    }

    pub fn trashed_message_ids(&self) -> plain_libs::rusqlite::Result<Vec<String>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare("SELECT CASE WHEN is_mms=1 THEN 'mms_' || message_id ELSE message_id END FROM trashed_sms")?;
            stmt.query_map([], |r| r.get(0))?.collect()
        })
    }

    pub fn trashed_message_save_many(&self, rows: &[TrashedMessageRow]) -> plain_libs::rusqlite::Result<usize> {
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
                        plain_libs::utils::dbtime::normalize(&row.trashed_at)?
                    ])?;
                }
            }
            tx.commit()?;
            Ok(inserted)
        })
    }

    pub fn trashed_message_delete_by_ids(&self, ids: &[String]) -> plain_libs::rusqlite::Result<usize> {
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

    pub fn trashed_message_delete_older_than(&self, before: &str) -> plain_libs::rusqlite::Result<usize> {
        self.with_conn(|c| c.execute("DELETE FROM trashed_sms WHERE trashed_at < ?1", [before]))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/simple.rs"]
mod simple_tests;
