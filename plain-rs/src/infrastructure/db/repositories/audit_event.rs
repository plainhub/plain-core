use crate::features::audit_events::AuditEvent;
use crate::infrastructure::db::{AuditEventClient, AuditEventRow, Db};
use plain_libs::rusqlite::{params, types::Type};
use plain_libs::serde_json::{Value, json};

impl Db {
    pub fn audit_event_client(
        &self,
        client_id: &str,
    ) -> plain_libs::rusqlite::Result<AuditEventClient> {
        let row = self
            .session_get(client_id)?
            .ok_or(plain_libs::rusqlite::Error::QueryReturnedNoRows)?;
        Ok(AuditEventClient {
            client_id: row.client_id,
            client_name: row.name,
            client_ip: row.client_ip,
            os_name: row.os_name,
            os_version: row.os_version,
            app_name: row.app_name,
            app_version: row.app_version,
        })
    }

    pub fn audit_event_add(
        &self,
        event: &AuditEvent,
        client: Option<&AuditEventClient>,
        created_at: &str,
    ) -> plain_libs::rusqlite::Result<String> {
        let encoded = plain_libs::serde_json::to_value(event).map_err(|error| {
            plain_libs::rusqlite::Error::ToSqlConversionFailure(Box::new(error))
        })?;
        let kind = encoded["type"].as_str().unwrap();
        let data = encoded["data"].to_string();
        let id = crate::features::record_id::generate();
        let field = |read: fn(&AuditEventClient) -> &str| {
            client.map(read).filter(|value| !value.is_empty())
        };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO events(id,type,data,client_id,client_name,client_ip,os_name,os_version,app_name,app_version,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![id, kind, data, field(|c| &c.client_id), field(|c| &c.client_name), field(|c| &c.client_ip), field(|c| &c.os_name), field(|c| &c.os_version), field(|c| &c.app_name), field(|c| &c.app_version), created_at],
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
            let mut statement = conn.prepare("SELECT id,type,data,client_id,client_name,client_ip,os_name,os_version,app_name,app_version,created_at FROM events ORDER BY created_at DESC,id DESC")?;
            let mut rows = statement.query([])?;
            let mut result = Vec::new();
            let mut skipped = 0;
            while let Some(row) = rows.next()? {
                let kind: String = row.get(1)?;
                let raw: String = row.get(2)?;
                let data: Value = plain_libs::serde_json::from_str(&raw)
                    .map_err(|error| plain_libs::rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(error)))?;
                let event: AuditEvent = plain_libs::serde_json::from_value(json!({"type":kind,"data":data.clone()}))
                    .map_err(|error| plain_libs::rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(error)))?;
                let record = AuditEventRow {
                    id: row.get(0)?, r#type: event.kind(), data: event.data(), client_id: row.get(3)?,
                    client_name: row.get(4)?, client_ip: row.get(5)?, os_name: row.get(6)?, os_version: row.get(7)?,
                    app_name: row.get(8)?, app_version: row.get(9)?, created_at: row.get(10)?,
                };
                let matches = needle.as_ref().is_none_or(|text| {
                    [record.id.as_str(), &kind, record.client_id.as_deref().unwrap_or_default(), record.client_name.as_deref().unwrap_or_default(), record.client_ip.as_deref().unwrap_or_default(), record.os_name.as_deref().unwrap_or_default(), record.os_version.as_deref().unwrap_or_default(), record.app_name.as_deref().unwrap_or_default(), record.app_version.as_deref().unwrap_or_default()]
                        .join(" ").to_lowercase().contains(text) || payload_contains(&data, text)
                });
                if !matches { continue; }
                if skipped < offset { skipped += 1; continue; }
                if result.len() == limit { break; }
                result.push(record);
            }
            Ok(result)
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/audit_event.rs"]
mod tests;

fn payload_contains(value: &Value, text: &str) -> bool {
    match value {
        Value::String(value) => value.to_lowercase().contains(text),
        Value::Array(values) => values.iter().any(|value| payload_contains(value, text)),
        Value::Object(values) => values.values().any(|value| payload_contains(value, text)),
        Value::Number(value) => value.to_string().contains(text),
        _ => false,
    }
}
