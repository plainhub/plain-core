use crate::db::{Db, SessionRow};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionInfo {
    pub client_id: String,
    pub token: String,
    pub client_name: String,
    pub browser_name: String,
    pub browser_version: String,
    pub os_name: String,
    pub os_version: String,
    pub last_active: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<SessionRow> for SessionInfo {
    type Error = anyhow::Error;

    fn try_from(row: SessionRow) -> Result<Self> {
        Ok(Self {
            client_id: row.client_id,
            token: row.token,
            client_name: row.name,
            browser_name: row.browser_name,
            browser_version: row.browser_version,
            os_name: row.os_name,
            os_version: row.os_version,
            last_active: DateTime::parse_from_rfc3339(
                row.last_seen_at.as_deref().unwrap_or(&row.updated_at),
            )?
            .with_timezone(&Utc),
            created_at: DateTime::parse_from_rfc3339(&row.created_at)?.with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.updated_at)?.with_timezone(&Utc),
        })
    }
}

pub struct SessionStore<'a> {
    db: &'a Db,
}

impl<'a> SessionStore<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn get(&self, client_id: &str) -> Result<Option<SessionInfo>> {
        self.db
            .session_get(client_id)?
            .map(TryInto::try_into)
            .transpose()
    }

    pub fn upsert(&self, mut info: SessionInfo) -> Result<SessionInfo> {
        if info.client_id.is_empty() {
            return Err(anyhow!("session client identity required"));
        }
        if info.token.is_empty() {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            info.token = crate::utils::base64::base64_encode(&key);
        }
        token_key(&info.token)?;
        let existing = self.db.session_get(&info.client_id)?;
        let now = Utc::now();
        if info.created_at.timestamp() == 0 {
            info.created_at = now;
        }
        info.updated_at = now;
        info.last_active = now;
        self.db.session_save(&SessionRow {
            url_token: existing
                .as_ref()
                .filter(|row| row.token == info.token)
                .map(|row| row.url_token.clone())
                .unwrap_or_else(crate::crypto::gen_token),
            client_id: info.client_id.clone(),
            token: info.token.clone(),
            name: info.client_name.clone(),
            browser_name: info.browser_name.clone(),
            browser_version: info.browser_version.clone(),
            os_name: info.os_name.clone(),
            os_version: info.os_version.clone(),
            client_ip: existing
                .as_ref()
                .map(|row| row.client_ip.clone())
                .unwrap_or_default(),
            r#type: existing
                .map(|row| row.r#type)
                .unwrap_or_else(|| "CUSTOM".into()),
            last_seen_at: Some(info.last_active.to_rfc3339()),
            created_at: info.created_at.to_rfc3339(),
            updated_at: info.updated_at.to_rfc3339(),
        })?;
        Ok(info)
    }

    pub fn touch_last_active(&self, info: &SessionInfo) -> Result<()> {
        self.db
            .session_touch(&[(info.client_id.clone(), Utc::now().to_rfc3339())])?;
        Ok(())
    }

    pub fn delete(&self, client_id: &str) -> Result<()> {
        self.db.session_delete(client_id)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<SessionInfo>> {
        self.db
            .session_list()?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }
}

pub fn token_key(token: &str) -> Result<[u8; 32]> {
    crate::utils::base64::base64_decode_checked(token)
        .map_err(|_| anyhow!("invalid base64 token"))?
        .try_into()
        .map_err(|_| anyhow!("session token must contain 32 bytes"))
}

#[cfg(test)]
#[path = "../tests/unit/sessions.rs"]
mod tests;
