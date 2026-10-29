use crate::db::Db;
use crate::enums::DataType;
use crate::error::DomainError;
use crate::error::DomainResult;
use crate::media::audio::queue as audio_queue;
use rusqlite::{OptionalExtension, Transaction, params};
use std::collections::HashSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[cfg_attr(feature = "graphql", graphql(name = "MediaAction"))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Trash,
    Restore,
    Delete,
    Move,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub path: String,
    pub destination_path: String,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub successful: Vec<Item>,
    pub failed_ids: Vec<String>,
}
pub fn validate(
    kind: DataType,
    action: Action,
    requested: &[String],
    outcome: &Outcome,
) -> DomainResult<()> {
    if !matches!(
        kind,
        DataType::Audio | DataType::Video | DataType::Image | DataType::Doc
    ) || requested.len() > 500
    {
        return Err(invalid("invalid media action"));
    }
    let expected = requested.iter().collect::<HashSet<_>>();
    if expected.len() != requested.len() || requested.iter().any(|id| id.is_empty()) {
        return Err(invalid("invalid media selection"));
    }
    let mut actual = HashSet::new();
    for item in &outcome.successful {
        if !expected.contains(&item.id)
            || item.path.is_empty()
            || !actual.insert(&item.id)
            || (action == Action::Move && item.destination_path.is_empty())
        {
            return Err(invalid("invalid media action receipt"));
        }
    }
    for id in &outcome.failed_ids {
        if !expected.contains(id) || !actual.insert(id) {
            return Err(invalid("invalid media failure receipt"));
        }
    }
    if actual.len() != expected.len() {
        return Err(invalid("incomplete media action receipt"));
    }
    Ok(())
}
pub fn cleanup(
    db: &Db,
    kind: DataType,
    action: Action,
    successful: &[Item],
) -> DomainResult<usize> {
    if !matches!(
        kind,
        DataType::Audio | DataType::Video | DataType::Image | DataType::Doc | DataType::File
    ) {
        return Err(invalid("unsupported media kind"));
    }
    if successful.is_empty() {
        return Ok(successful.len());
    }
    let ids = serde_json::to_string(&successful.iter().map(|item| &item.id).collect::<Vec<_>>())
        .map_err(|e| invalid(&e.to_string()))?;
    let paths = successful
        .iter()
        .map(|item| item.path.clone())
        .collect::<Vec<_>>();
    db.audio_transaction(|tx, state| {
        if kind==DataType::Audio && action==Action::Delete {
            for table in ["audio_queue_items","audio_playlist_items"] {
                tx.execute(&format!("DELETE FROM {table} WHERE file_id IN (SELECT value FROM json_each(?1))"),[&ids])?;
            }
        }
        for item in successful { update_catalog(&tx, kind, action, item)?; }
        if action==Action::Delete {
            tx.execute("DELETE FROM tag_relations WHERE type=?1 AND target_key IN (SELECT value FROM json_each(?2))",params![kind.kind(),ids])?;
            if matches!(kind,DataType::Audio|DataType::Video) {
                tx.execute("DELETE FROM media_play_history WHERE media_type=?2 AND file_id IN (SELECT value FROM json_each(?1))",params![ids,kind.kind()])?;
            }
        }
        if kind==DataType::Image && matches!(action,Action::Delete|Action::Move) {
            tx.execute("DELETE FROM image_embeddings WHERE file_id IN (SELECT value FROM json_each(?1))",[&ids])?;
        }
        if kind==DataType::Audio && action==Action::Delete { audio_queue::remove_paths_conn(&tx,&paths, state)?; }
        if kind==DataType::Audio && action==Action::Trash && successful.iter().any(|item|state.current_file_id.as_deref()==Some(&item.id)) { state.select("")?; }
        if kind==DataType::Audio && action==Action::Move {
            if let Some(item)=successful.iter().find(|item|item.path==state.playback.path) {
                crate::db::audio_queue::io::save_current(tx,&item.destination_path,state)?;
                state.select(&item.destination_path)?;
            }
        }

        Ok(successful.len())
    })
}
fn update_catalog(
    tx: &Transaction<'_>,
    kind: DataType,
    action: Action,
    item: &Item,
) -> rusqlite::Result<()> {
    let row: Option<(i64,String,Option<String>,bool)> = tx.query_row(
        "SELECT file_id,path,original_path,trashed FROM file_catalog WHERE id=?1 AND data_type=?2",
        params![item.id,kind.kind()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).optional()?;
    let Some((id, path, original, trashed)) = row else {
        return Ok(());
    };
    if path != item.path {
        if path == item.destination_path
            && matches!(
                (action, trashed),
                (Action::Trash, true) | (Action::Restore, false)
            )
        {
            return Ok(());
        }
        return Err(rusqlite::Error::InvalidQuery);
    }
    if action == Action::Delete {
        tx.execute("DELETE FROM files WHERE id=?1", [id])?;
        return Ok(());
    }
    let destination = match action {
        Action::Trash if !trashed => {
            if item.destination_path.is_empty() {
                path.clone()
            } else {
                item.destination_path.clone()
            }
        }
        Action::Restore if trashed => {
            if item.destination_path.is_empty() {
                original.ok_or(rusqlite::Error::InvalidQuery)?
            } else {
                item.destination_path.clone()
            }
        }
        Action::Move if !trashed && !item.destination_path.is_empty() => {
            item.destination_path.clone()
        }
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let location = crate::db::files::locate(&destination)?;
    let (volume_id, relative) = crate::db::files::bind_volume(tx, &location)?;
    let previous: (i64, String) = tx.query_row(
        "SELECT volume_id,path FROM files WHERE id=?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if action == Action::Trash && volume_id != previous.0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let name = location
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let (time, original) = if action == Action::Trash {
        (
            Some(crate::utils::dbtime::now_iso_millis()),
            Some(previous.1),
        )
    } else {
        (None, None)
    };
    tx.execute("UPDATE files SET volume_id=?1,path=?2,file_name=?3,trashed_at=?4,original_path=?5,missing=0 WHERE id=?6",params![volume_id,relative,name,time,original,id])?;
    Ok(())
}

fn invalid(message: &str) -> DomainError {
    DomainError::Other(message.into())
}
#[cfg(test)]
#[path = "../../tests/unit/media/actions.rs"]
mod tests;
