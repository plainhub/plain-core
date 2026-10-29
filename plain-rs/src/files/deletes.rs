use crate::db::Db;
use crate::enums::DataType;
use crate::error::DomainError;
use crate::error::DomainResult;
use crate::media::audio::queue as audio_queue;
use rusqlite::params;
#[derive(Clone)]
pub struct Item {
    pub media_type: i32,
    pub id: String,
    pub path: String,
}
pub fn cleanup(db: &Db, items: &[Item], paths: &[String], roots: &[String]) -> DomainResult<()> {
    if items.iter().any(|item| {
        !matches!(item.media_type, 1 | 2 | 3 | 22 | 24)
            || item.id.is_empty()
            || !paths.contains(&item.path)
    }) {
        return Err(DomainError::Other("invalid deleted media receipt".into()));
    }
    db.audio_transaction(|tx, state| {
        let encoded=serde_json::to_string(paths).map_err(|e|DomainError::Other(e.to_string()))?;
        let mut media = items.to_vec();
        {
            let mut statement=tx.prepare("SELECT data_type,id,path FROM file_catalog WHERE path IN (SELECT value FROM json_each(?1))")?;
            media.extend(statement.query_map([&encoded],|row|Ok(Item { media_type:row.get(0)?,id:row.get(1)?,path:row.get(2)? }))?.collect::<rusqlite::Result<Vec<_>>>()?);
        }
        for root in roots {
            let prefix=directory_prefix(root);
            let mut statement=tx.prepare("SELECT data_type,id,path FROM file_catalog WHERE path=?1 OR substr(path,1,length(?2))=?2")?;
            media.extend(statement.query_map(params![root,prefix],|row|Ok(Item { media_type:row.get(0)?,id:row.get(1)?,path:row.get(2)? }))?.collect::<rusqlite::Result<Vec<_>>>()?);
        }
        let mut seen=std::collections::HashSet::new();
        for item in &media {
            if !seen.insert((item.media_type,&item.id)) { continue; }
            remove_identity(tx, item)?;
        }
        audio_queue::remove_paths_conn(&tx,paths, state)?;
        tx.execute("DELETE FROM files WHERE id IN (SELECT file_id FROM file_catalog WHERE path IN (SELECT value FROM json_each(?1)))",[&encoded])?;
        tx.execute("DELETE FROM tag_relations WHERE type=?1 AND target_key IN (SELECT value FROM json_each(?2))",params![DataType::File.kind(),encoded])?;
        tx.execute("DELETE FROM image_embeddings WHERE path IN (SELECT value FROM json_each(?1))",[&encoded])?;
        for root in roots {
            let prefix=directory_prefix(root);
            audio_queue::remove_root_paths_conn(&tx,root, state)?;
            tx.execute("DELETE FROM files WHERE id IN (SELECT file_id FROM file_catalog WHERE path=?1 OR substr(path,1,length(?2))=?2)",params![root,prefix])?;
            tx.execute("DELETE FROM tag_relations WHERE type=?1 AND (target_key=?2 OR substr(target_key,1,length(?3))=?3)",params![DataType::File.kind(),root,prefix])?;
            tx.execute("DELETE FROM image_embeddings WHERE path=?1 OR substr(path,1,length(?2))=?2",params![root,prefix])?;
        }
        Ok(())
    })
}
pub fn retire_identity(db: &Db, item: &Item) -> DomainResult<()> {
    if !matches!(item.media_type, 1 | 2 | 3 | 22 | 24) || item.id.is_empty() || item.path.is_empty()
    {
        return Err(DomainError::Other("invalid retired media identity".into()));
    }
    db.audio_transaction(|tx, state| {
        if item.media_type == DataType::Audio.kind()
            && state.current_file_id.as_deref() == Some(item.id.as_str())
        {
            state.select("")?;
        }
        remove_identity(tx, item)?;
        tx.execute(
            "DELETE FROM files WHERE id IN (SELECT file_id FROM file_catalog WHERE data_type=?1 AND id=?2 AND path=?3)",
            params![item.media_type, item.id, item.path],
        )?;
        Ok(())
    })
}

fn remove_identity(tx: &rusqlite::Connection, item: &Item) -> DomainResult<()> {
    tx.execute(
        "DELETE FROM tag_relations WHERE type=?1 AND target_key=?2",
        params![item.media_type, item.id],
    )?;
    if item.media_type == 1 {
        for table in ["audio_queue_items", "audio_playlist_items"] {
            tx.execute(&format!("DELETE FROM {table} WHERE file_id=?1"), [&item.id])?;
        }
    }
    match item.media_type {
        1 | 2 => {
            tx.execute(
                "DELETE FROM media_play_history WHERE file_id=?1 AND media_type=?2",
                params![item.id, item.media_type],
            )?;
        }
        3 => {
            tx.execute("DELETE FROM image_embeddings WHERE file_id=?1", [&item.id])?;
        }
        _ => {}
    }
    Ok(())
}

fn directory_prefix(root: &str) -> String {
    let separator = if cfg!(windows) && !root.contains('/') {
        '\\'
    } else {
        '/'
    };
    format!("{}{separator}", root.trim_end_matches(separator))
}

#[cfg(test)]
#[path = "../../tests/unit/files/deletes.rs"]
mod tests;
