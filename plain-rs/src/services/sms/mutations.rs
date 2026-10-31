use anyhow::{Result, anyhow, bail};
use plain_rs::host::bridge::Host;
use plain_rs::services::sms::query;
use plain_rs::services::sms::query::Request;
use plain_rs::{
    db::{Db, TrashedMessageRow},
    prefs::Prefs,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone, Copy)]
pub enum Action {
    Trash,
    Restore,
    Delete,
}

pub struct Mutations {
    db: Arc<Db>,
    host: Arc<Host>,
    prefs: Arc<Prefs>,
    lock: Mutex<()>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    deleted_ids: Vec<String>,
    failed_ids: Vec<String>,
}

struct PendingTrash {
    db: Arc<Db>,
    ids: HashSet<String>,
}

impl PendingTrash {
    fn rollback(&mut self, ids: &[String]) -> Result<()> {
        let ids = ids
            .iter()
            .filter(|id| self.ids.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        self.db.trashed_message_delete_by_ids(&ids)?;
        for id in ids {
            self.ids.remove(&id);
        }
        Ok(())
    }
}

impl Drop for PendingTrash {
    fn drop(&mut self) {
        if !self.ids.is_empty() {
            if let Err(error) = self
                .db
                .trashed_message_delete_by_ids(&self.ids.iter().cloned().collect::<Vec<_>>())
            {
                log::error!("SMS deletion rollback failed: {error}");
            }
        }
    }
}

impl Mutations {
    pub fn new(db: Arc<Db>, host: Arc<Host>, prefs: Arc<Prefs>) -> Arc<Self> {
        Arc::new(Self {
            db,
            host,
            prefs,
            lock: Mutex::new(()),
        })
    }

    pub async fn execute(&self, action: Action, filter: String) -> Result<i32> {
        if filter.trim().is_empty() {
            bail!(
                "query is required for bulk mutations — pass 'all:true' to explicitly target everything"
            );
        }
        let _guard = self.lock.lock().await;
        let result = query::execute(
            &self.db,
            &self.host,
            Request::Ids {
                query: filter,
                include_trashed: !matches!(action, Action::Trash),
            },
        )
        .await?;
        let ids: Vec<String> = serde_json::from_value(result["ids"].clone())?;
        for id in &ids {
            let raw = id.strip_prefix("mms_").unwrap_or(id);
            if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
                bail!("invalid SMS provider identity");
            }
        }
        let count = match action {
            Action::Trash => save_trash(&self.db, &ids)?,
            Action::Restore => self.db.trashed_message_delete_by_ids(&ids)?,
            Action::Delete => self.delete(ids).await?,
        };
        Ok(i32::try_from(count)?)
    }

    async fn delete(&self, ids: Vec<String>) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let facts = query::execute(
            &self.db,
            &self.host,
            Request::Search {
                query: format!("ids:{}", ids.join(",")),
                offset: 0,
                limit: i32::try_from(ids.len())?,
                include_trashed: true,
            },
        )
        .await?;
        let messages: Vec<Value> = serde_json::from_value(facts["items"].clone())?;
        let selected = ids.iter().map(String::as_str).collect::<HashSet<_>>();
        let mut found = HashSet::new();
        for row in &messages {
            let id = row["id"]
                .as_str()
                .ok_or_else(|| anyhow!("SMS archive identity missing"))?;
            if !selected.contains(id) || !found.insert(id) {
                bail!("invalid SMS archive provider rows");
            }
        }
        if found != selected {
            bail!("SMS archive provider rows incomplete");
        }
        let directory = self
            .prefs
            .path()
            .parent()
            .ok_or_else(|| anyhow!("private preferences directory unavailable"))?
            .join("sms-trash");
        tokio::task::spawn_blocking(move || {
            plain_rs::services::sms::archives::write(
                &directory,
                &messages,
                chrono::Utc::now().timestamp_millis(),
            )
        })
        .await??;
        let known = self
            .db
            .trashed_message_ids()?
            .into_iter()
            .collect::<HashSet<_>>();
        let mut pending = PendingTrash {
            db: self.db.clone(),
            ids: ids
                .iter()
                .filter(|id| !known.contains(*id))
                .cloned()
                .collect(),
        };
        save_trash(&self.db, &ids)?;
        let result = self
            .host
            .call("systemDeleteSmsRows", json!({"ids":ids}))
            .await
            .map_err(anyhow::Error::msg)
            .and_then(|facts| serde_json::from_value::<Receipt>(facts).map_err(Into::into));
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => {
                pending
                    .rollback(&ids)
                    .map_err(|rollback| anyhow!("{error}; SMS rollback failed: {rollback}"))?;
                return Err(error);
            }
        };
        let deleted = receipt
            .deleted_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let failed = receipt
            .failed_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if deleted.len() != receipt.deleted_ids.len()
            || failed.len() != receipt.failed_ids.len()
            || !deleted.is_disjoint(&failed)
            || deleted.union(&failed).copied().collect::<HashSet<_>>() != selected
        {
            pending.rollback(&ids)?;
            bail!("invalid SMS deletion receipt");
        }
        pending.rollback(&receipt.failed_ids)?;
        pending.ids.clear();
        Ok(receipt.deleted_ids.len())
    }
}

fn save_trash(db: &Db, ids: &[String]) -> Result<usize> {
    let now = plain_rs::utils::dbtime::now_iso();
    let rows = ids
        .iter()
        .map(|id| {
            let (message_id, is_mms) = id
                .strip_prefix("mms_")
                .map(|raw| (raw, true))
                .unwrap_or((id.as_str(), false));
            TrashedMessageRow {
                message_id: message_id.into(),
                is_mms,
                trashed_at: now.clone(),
            }
        })
        .collect::<Vec<_>>();
    Ok(db.trashed_message_save_many(&rows)?)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/mutations.rs"]
mod tests;
