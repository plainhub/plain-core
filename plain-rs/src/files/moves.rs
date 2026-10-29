use super::audio_moves;
use crate::db::Db;
use crate::enums::DataType;
use crate::error::DomainError;
use crate::error::DomainResult;
use crate::media::audio::queue as audio_queue;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub media_type: i32,
    pub source_id: String,
    pub destination_id: String,
    pub source_path: String,
    pub destination_path: String,
}
fn kind(value: i32) -> DomainResult<DataType> {
    match value {
        1 => Ok(DataType::Audio),
        2 => Ok(DataType::Video),
        3 => Ok(DataType::Image),
        24 => Ok(DataType::Doc),
        22 => Ok(DataType::File),
        _ => Err(invalid("unsupported media move type")),
    }
}
pub fn rebind(
    db: &Db,
    bindings: &[Binding],
    source_root: &str,
    destination_root: &str,
) -> DomainResult<usize> {
    rebind_with_aliases(db, bindings, source_root, destination_root, &[])
}
pub fn rebind_with_aliases(
    db: &Db,
    bindings: &[Binding],
    source_root: &str,
    destination_root: &str,
    aliases: &[(String, String)],
) -> DomainResult<usize> {
    let mut sources = HashSet::new();
    let mut destinations = HashSet::new();
    for binding in bindings {
        kind(binding.media_type)?;
        if binding.source_id.is_empty()
            || binding.destination_id.is_empty()
            || binding.source_path.is_empty()
            || binding.destination_path.is_empty()
            || !sources.insert((binding.media_type, binding.source_id.as_str()))
            || !destinations.insert((binding.media_type, binding.destination_id.as_str()))
        {
            return Err(invalid("invalid media move binding"));
        }
    }
    if source_root.is_empty() || destination_root.is_empty() {
        return Err(invalid("file move roots required"));
    }
    db.audio_transaction(|tx, state| {
        let mut snapshots = Vec::new();
        let current_path = state.playback.path.clone();
        for binding in bindings {
            let media_type = kind(binding.media_type)?;
            let mut statement = tx.prepare("SELECT tag_id,created_at FROM tag_relations WHERE type=?1 AND target_key=?2")?;
            let tags = statement.query_map(params![binding.media_type,binding.source_id],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            let progress = if matches!(media_type,DataType::Audio|DataType::Video) {
                tx.query_row("SELECT position_ms,played_at,play_count FROM media_play_history WHERE file_id=?1 AND media_type=?2",params![binding.source_id,binding.media_type],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?))).optional()?
            } else { None };
            let references = if media_type==DataType::Audio { Some(audio_moves::snapshot(tx,&binding.source_id)?) } else { None };
            snapshots.push((binding,media_type,tags,progress,references));
        }
        for (binding,media_type,_,_,_) in &snapshots {
            if binding.source_id==binding.destination_id { continue; }
            if *media_type==DataType::Audio { audio_moves::remove_source(tx,&binding.source_id)?; }
            tx.execute("DELETE FROM tag_relations WHERE type=?1 AND target_key=?2",params![binding.media_type,binding.source_id])?;
            if matches!(media_type,DataType::Audio|DataType::Video) { tx.execute("DELETE FROM media_play_history WHERE file_id=?1 AND media_type=?2",params![binding.source_id,binding.media_type])?; }
        }
        for (binding,media_type,tags,progress,references) in snapshots {
            if binding.source_id!=binding.destination_id {
                for (tag,created) in tags {
                    tx.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES(?1,?2,?3,?4) ON CONFLICT(tag_id,target_key,type) DO UPDATE SET created_at=min(tag_relations.created_at,excluded.created_at)",params![tag,binding.destination_id,binding.media_type,created])?;
                }
                if matches!(media_type,DataType::Audio|DataType::Video) {
                    tx.execute("DELETE FROM media_play_history WHERE file_id=?1 AND media_type=?2",params![binding.destination_id,binding.media_type])?;
                    if let Some((position,played,count)) = progress {
                        tx.execute("INSERT INTO media_play_history(file_id,position_ms,play_count,played_at,media_type) VALUES(?1,?2,?3,?4,?5)",params![binding.destination_id,position,count,played,binding.media_type])?;
                    }
                }
            }
            if media_type==DataType::Image { tx.execute("DELETE FROM image_embeddings WHERE file_id IN (?1,?2)",params![binding.source_id,binding.destination_id])?; }
            if media_type==DataType::Audio {
                if binding.source_id!=binding.destination_id {
                    audio_moves::apply(tx,&binding.destination_id,references.unwrap())?;
                    if state.current_file_id.as_deref() == Some(binding.source_id.as_str()) { state.current_file_id = Some(binding.destination_id.clone()); }
                }
                if current_path==binding.source_path { state.select(&binding.destination_path)?; }
            }
        }
        let retained = bindings.iter().flat_map(|binding|[binding.source_id.clone(),binding.destination_id.clone()]).collect::<Vec<_>>();
        let mut roots=HashSet::new();
        for (source_root,destination_root) in aliases.iter().map(|(a,b)|(a.as_str(),b.as_str())).chain(std::iter::once((source_root,destination_root))) {
            if source_root.is_empty()||destination_root.is_empty() { return Err(invalid("file move roots required")); }
            if !roots.insert(source_root) { continue; }
        audio_queue::remove_root_paths_except_conn(tx,source_root,state,&retained)?;

        let source_prefix = format!("{}/",source_root.trim_end_matches('/'));
        let mut statement = tx.prepare("SELECT tag_id,target_key,created_at FROM tag_relations WHERE type=?1 AND (target_key=?2 OR substr(target_key,1,length(?3))=?3)")?;
        let relations = statement.query_map(params![DataType::File.kind(),source_root,source_prefix],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        for (tag,key,created) in relations {
            let relative = if key==source_root { "" } else { &key[source_prefix.len()..] };
            let destination = if relative.is_empty() { destination_root.to_owned() } else { format!("{}/{relative}",destination_root.trim_end_matches('/')) };
            if destination==key { continue; }
            tx.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES(?1,?2,?3,?4) ON CONFLICT(tag_id,target_key,type) DO UPDATE SET created_at=min(tag_relations.created_at,excluded.created_at)",params![tag,destination,DataType::File.kind(),created])?;
            tx.execute("DELETE FROM tag_relations WHERE tag_id=?1 AND target_key=?2 AND type=?3",params![tag,key,DataType::File.kind()])?;
        }
        tx.execute("DELETE FROM image_embeddings WHERE path=?1 OR substr(path,1,length(?2))=?2",params![source_root,source_prefix])?;
        }

        Ok(bindings.len())
    })
}
fn invalid(message: &str) -> DomainError {
    DomainError::Other(message.into())
}
#[cfg(test)]
#[path = "../../tests/unit/files/moves.rs"]
mod tests;
