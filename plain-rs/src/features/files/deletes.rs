use crate::features::enums::DataType;
use crate::features::media::audio::queue as audio_queue;
use crate::infrastructure::db::Db;
use crate::infrastructure::error::DomainError;
use crate::infrastructure::error::DomainResult;
use rusqlite::params;
#[derive(Clone, serde::Serialize)]
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
        media.retain(|item| seen.insert((item.media_type, item.id.clone())));
        remove_identities(tx, &media)?;
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
        remove_identities(tx, std::slice::from_ref(item))?;
        tx.execute(
            "DELETE FROM files WHERE id IN (SELECT file_id FROM file_catalog WHERE data_type=?1 AND id=?2 AND path=?3)",
            params![item.media_type, item.id, item.path],
        )?;
        Ok(())
    })
}

pub(crate) fn reconcile_scan_batch(
    db: &Db,
    root: &std::path::Path,
    scan_id: &str,
    skipped: &[std::path::PathBuf],
    limit: u32,
    after: &str,
) -> DomainResult<Vec<Item>> {
    let root = root
        .to_str()
        .ok_or_else(|| DomainError::Other("Invalid scan root encoding".into()))?
        .replace('\\', "/");
    let skipped = skipped
        .iter()
        .map(|path| {
            path.to_str()
                .map(|path| path.replace('\\', "/"))
                .ok_or_else(|| DomainError::Other("Invalid skipped path encoding".into()))
        })
        .collect::<DomainResult<Vec<_>>>()?;
    let skipped =
        serde_json::to_string(&skipped).map_err(|error| DomainError::Other(error.to_string()))?;
    db.audio_transaction(|tx, state| {
        let rows = tx.prepare("SELECT c.data_type,c.id,c.path FROM file_catalog c JOIN files f ON f.id=c.id
WHERE f.id>?6 AND f.last_seen_scan_id IS NOT ?1 AND c.is_available=1 AND c.trashed=0
AND (replace(c.path,char(92),'/')=?2 OR substr(replace(c.path,char(92),'/'),1,length(?3))=?3)
AND NOT EXISTS(SELECT 1 FROM json_each(?4) s WHERE replace(c.path,char(92),'/')=s.value OR substr(replace(c.path,char(92),'/'),1,length(rtrim(s.value,'/')||'/'))=rtrim(s.value,'/')||'/')
ORDER BY f.id LIMIT ?5")?.query_map(params![scan_id, root, format!("{}/",root.trim_end_matches('/')), skipped, limit, after], |row| {
            Ok(Item { media_type: row.get(0)?, id: row.get(1)?, path: row.get(2)? })
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        if rows.is_empty() { return Ok(Vec::new()); }
        if rows.iter().any(|item| state.current_file_id.as_deref() == Some(item.id.as_str())) {
            state.select("")?;
        }
        remove_identities(tx, &rows)?;
        let encoded = serde_json::to_string(&rows).map_err(|error| DomainError::Other(error.to_string()))?;
        tx.execute("DELETE FROM files WHERE id IN (SELECT value->>'id' FROM json_each(?1))", [&encoded])?;
        tx.execute("DELETE FROM tag_relations WHERE type=22 AND target_key IN (SELECT value->>'path' FROM json_each(?1)) AND NOT EXISTS(SELECT 1 FROM file_catalog WHERE path=target_key)", [&encoded])?;
        Ok(rows)
    })
}

fn remove_identities(tx: &rusqlite::Connection, items: &[Item]) -> DomainResult<()> {
    let encoded =
        serde_json::to_string(items).map_err(|error| DomainError::Other(error.to_string()))?;
    tx.execute("DELETE FROM tag_relations WHERE target_key IN (SELECT value->>'id' FROM json_each(?1)) AND EXISTS(SELECT 1 FROM json_each(?1) i WHERE i.value->>'media_type'=tag_relations.type AND i.value->>'id'=tag_relations.target_key)", [&encoded])?;
    for table in ["audio_queue_items", "audio_playlist_items"] {
        tx.execute(&format!("DELETE FROM {table} WHERE file_id IN (SELECT value->>'id' FROM json_each(?1) WHERE value->>'media_type'=1)"), [&encoded])?;
    }
    tx.execute("DELETE FROM media_play_history WHERE media_type IN (1,2) AND file_id IN (SELECT value->>'id' FROM json_each(?1)) AND EXISTS(SELECT 1 FROM json_each(?1) i WHERE i.value->>'media_type'=media_type AND i.value->>'id'=file_id)", [&encoded])?;
    tx.execute("DELETE FROM image_embeddings WHERE file_id IN (SELECT value->>'id' FROM json_each(?1) WHERE value->>'media_type'=3)", [&encoded])?;
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
#[path = "../../../../tests/plain-rs/unit/features/files/deletes.rs"]
mod tests;
