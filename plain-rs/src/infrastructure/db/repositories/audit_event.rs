use crate::infrastructure::db::AuditEventRow;
use crate::infrastructure::db::Db;
use rusqlite::params;

impl Db {
    pub fn audit_event_add(
        &self,
        kind: &str,
        message: &str,
        client_id: Option<&str>,
    ) -> rusqlite::Result<String> {
        let id = crate::features::record_id::generate();
        let at = plain_libs::chrono::Utc::now().to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true);
        self.with_conn(|conn| {
            conn.execute(
            "INSERT INTO audit_events(id,type,message,client_id,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![id,kind,message,client_id,at],
        )
        })?;
        Ok(id)
    }

    pub fn audit_events(
        &self,
        offset: usize,
        limit: usize,
        needle: Option<&str>,
    ) -> rusqlite::Result<Vec<AuditEventRow>> {
        let needle = needle.map(str::to_lowercase);
        self.with_conn(|conn| {
            let mut statement = conn.prepare("SELECT id,type,message,client_id,created_at FROM audit_events ORDER BY created_at DESC,id DESC")?;
            let rows = statement.query_map([], |row| Ok(AuditEventRow {
                id: row.get(0)?, r#type: row.get(1)?, message: row.get(2)?,
                client_id: row.get(3)?, created_at: row.get(4)?,
            }))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows.into_iter().filter(|row| needle.as_ref().is_none_or(|text|
                format!("{} {}",row.r#type,row.message).to_lowercase().contains(text)
            )).skip(offset).take(limit).collect())
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/audit_event.rs"]
mod tests;
