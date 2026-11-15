use crate::infrastructure::db::{AuditEventClient, AuditEventRow, Db};
use plain_libs::rusqlite::params;

impl Db {
    pub fn audit_event_client(&self, client_id: &str) -> plain_libs::rusqlite::Result<AuditEventClient> {
        Ok(match self.session_get(client_id)? {
            Some(row) => AuditEventClient {
                client_id: row.client_id,
                client_name: row.name,
                client_ip: row.client_ip,
                os_name: row.os_name,
                os_version: row.os_version,
                app_name: row.app_name,
                app_version: row.app_version,
            },
            None => AuditEventClient { client_id: client_id.to_owned(), ..Default::default() },
        })
    }

    pub fn audit_event_add(
        &self,
        kind: &str,
        message: &str,
        client: Option<&AuditEventClient>,
    ) -> plain_libs::rusqlite::Result<String> {
        let id = crate::features::record_id::generate();
        let at = plain_libs::chrono::Utc::now().to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true);
        let empty = AuditEventClient::default();
        let facts = client.unwrap_or(&empty);
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO events(id,type,message,client_id,client_name,client_ip,os_name,os_version,app_name,app_version,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![id, kind, message, client.map(|facts| facts.client_id.as_str()), facts.client_name, facts.client_ip, facts.os_name, facts.os_version, facts.app_name, facts.app_version, at],
            )
        })?;
        Ok(id)
    }

    pub fn audit_events(
        &self,
        offset: usize,
        limit: usize,
        needle: Option<&str>,
    ) -> plain_libs::rusqlite::Result<Vec<AuditEventRow>> {
        let needle = needle.map(str::to_lowercase);
        self.with_conn(|conn| {
            let mut statement = conn.prepare("SELECT id,type,message,client_id,client_name,client_ip,os_name,os_version,app_name,app_version,created_at FROM events ORDER BY created_at DESC,id DESC")?;
            let rows = statement.query_map([], |row| Ok(AuditEventRow {
                id: row.get(0)?, r#type: row.get(1)?, message: row.get(2)?, client_id: row.get(3)?,
                client_name: row.get(4)?, client_ip: row.get(5)?, os_name: row.get(6)?, os_version: row.get(7)?,
                app_name: row.get(8)?, app_version: row.get(9)?, created_at: row.get(10)?,
            }))?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
            Ok(rows.into_iter().filter(|row| needle.as_ref().is_none_or(|text| {
                [&row.id, &row.r#type, &row.message, row.client_id.as_deref().unwrap_or_default(), &row.client_name, &row.client_ip, &row.os_name, &row.os_version, &row.app_name, &row.app_version]
                    .join(" ").to_lowercase().contains(text)
            })).skip(offset).take(limit).collect())
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/audit_event.rs"]
mod tests;
