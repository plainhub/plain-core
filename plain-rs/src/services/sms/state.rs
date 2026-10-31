use plain_rs::db::{ArchivedConversationRow, Db};

pub fn archive(db: &Db, id: &str, date: &str) -> anyhow::Result<()> {
    if id.is_empty() {
        anyhow::bail!("conversation id is empty");
    }
    let date = chrono::DateTime::parse_from_rfc3339(date)?
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    db.archived_conversation_save(&ArchivedConversationRow {
        conversation_id: id.into(),
        archived_at: date,
    })?;
    Ok(())
}

pub fn unarchive(db: &Db, id: &str) -> anyhow::Result<usize> {
    Ok(db.archived_conversation_delete(id)?)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/sms/state.rs"]
mod tests;
