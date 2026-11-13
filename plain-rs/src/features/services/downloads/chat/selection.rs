use super::CoreRuntime;
use crate::features::downloads::{Attachment, Source};
use plain_libs::{
    anyhow::{Result, ensure},
    serde_json::Value,
};
use std::collections::HashMap;

pub(crate) fn files(
    state: &CoreRuntime,
    message_id: &str,
    peer_id: &str,
    ids: &[String],
    completed: &HashMap<String, i64>,
) -> Result<Vec<Attachment>> {
    let row = crate::infrastructure::db::chat_store::messages::get(&state.db, message_id)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Message unavailable"))?;
    ensure!(
        peer_id == row.from_id || peer_id == row.to_id,
        "Attachment peer does not belong to message"
    );
    let content: Value = plain_libs::serde_json::from_str(&row.content)?;
    ensure!(
        matches!(content["type"].as_str(), Some("FILES" | "IMAGES")),
        "Message has no attachments"
    );
    let entries = content["value"]["items"]
        .as_array()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid message attachments"))?;
    ids.iter()
        .map(|id| {
            let mut matches = entries
                .iter()
                .filter(|file| file["id"].as_str() == Some(id));
            let value = matches
                .next()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Attachment unavailable"))?;
            ensure!(matches.next().is_none(), "Duplicate attachment identity");
            let file: Attachment = plain_libs::serde_json::from_value(value.clone())?;
            ensure!(
                completed.contains_key(id)
                    || file
                        .uri
                        .strip_prefix("fsid:")
                        .is_some_and(|id| !id.is_empty()),
                "Attachment is already local"
            );
            ensure!(file.size_bytes >= 0, "Invalid attachment size");
            if let Some(size) = completed.get(id) {
                ensure!(*size == file.size_bytes, "Completed attachment changed");
            }
            Ok(file)
        })
        .collect()
}

pub async fn received(
    state: &CoreRuntime,
    row: &crate::infrastructure::db::DChat,
    peer_id: &str,
) -> Result<()> {
    let content: Value = plain_libs::serde_json::from_str(&row.content)?;
    if !matches!(content["type"].as_str(), Some("FILES" | "IMAGES")) {
        return Ok(());
    }
    let files = content["value"]["items"]
        .as_array()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid message attachments"))?;
    for file in files {
        let Some(id) = file["id"].as_str() else {
            continue;
        };
        if let Err(error) = super::super::create(
            state,
            Source::Chat {
                message_id: row.id.clone(),
                peer_id: peer_id.into(),
                file_ids: vec![id.into()],
            },
            None,
            None,
        )
        .await
        {
            log::warn!("Incoming attachment queue: {error}");
        }
    }
    Ok(())
}
