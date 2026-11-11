use crate::infrastructure::db::ArchivedConversationRow;
use crate::infrastructure::db::Db;

pub fn archive(db: &Db, id: &str, date: &str) -> plain_libs::anyhow::Result<()> {
    if id.is_empty() {
        plain_libs::anyhow::bail!("conversation id is empty");
    }
    let date = plain_libs::chrono::DateTime::parse_from_rfc3339(date)?
        .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true);
    db.archived_conversation_save(&ArchivedConversationRow {
        conversation_id: id.into(),
        archived_at: date,
    })?;
    Ok(())
}

pub fn unarchive(db: &Db, id: &str) -> plain_libs::anyhow::Result<usize> {
    Ok(db.archived_conversation_delete(id)?)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/sms/state.rs"]
mod tests;
