use crate::features::enums::DataType;
use crate::infrastructure::db::Db;
pub use crate::infrastructure::db::models::tag::TagRelationRow;
pub use crate::infrastructure::db::models::tag::TagRow;
use plain_libs::rusqlite::{Connection, OptionalExtension, params};
const TAG_COLUMNS: &str =
    "id,name,type,(SELECT COUNT(*) FROM tag_relations r WHERE r.tag_id=tags.id)";
fn row_to_tag(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<TagRow> {
    Ok(TagRow {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: row.get(2)?,
        count: row.get(3)?,
    })
}
fn row_to_relation(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<TagRelationRow> {
    Ok(TagRelationRow {
        tag_id: row.get(0)?,
        target_key: row.get(1)?,
    })
}
fn transaction<T>(
    db: &Db,
    operation: impl FnOnce(&Connection) -> plain_libs::rusqlite::Result<T>,
) -> plain_libs::rusqlite::Result<T> {
    db.with_conn(|c| {
        let tx = c.unchecked_transaction()?;
        let result = operation(&tx)?;
        tx.commit()?;
        Ok(result)
    })
}
pub(crate) mod io {
    use super::*;
    pub fn tag_by_id(c: &Connection, id: &str) -> plain_libs::rusqlite::Result<Option<TagRow>> {
        c.query_row(
            &format!("SELECT {TAG_COLUMNS} FROM tags WHERE id=?"),
            [id],
            row_to_tag,
        )
        .optional()
    }
    pub fn insert_relations(c: &Connection, rels: &[(String, String)]) -> plain_libs::rusqlite::Result<()> {
        let mut statement=c.prepare("INSERT INTO tag_relations(tag_id,target_key,created_at) VALUES(?1,?2,?3) ON CONFLICT(tag_id,target_key) DO NOTHING")?;
        for (tag_id, key) in rels {
            if tag_id.is_empty() || key.is_empty() {
                continue;
            }
            statement.execute(params![
                tag_id,
                key,
                plain_libs::utils::dbtime::now_iso_millis()
            ])?;
        }
        Ok(())
    }
    pub fn remove_relations(
        c: &Connection,
        keys: &[String],
        tag_ids: &[String],
    ) -> plain_libs::rusqlite::Result<()> {
        if keys.is_empty() || tag_ids.is_empty() {
            return Ok(());
        }
        c.execute("DELETE FROM tag_relations WHERE target_key IN (SELECT value FROM json_each(?1)) AND tag_id IN (SELECT value FROM json_each(?2))",params![plain_libs::serde_json::to_string(keys).map_err(|e|plain_libs::rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,plain_libs::serde_json::to_string(tag_ids).map_err(|e|plain_libs::rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?])?;
        Ok(())
    }
}
pub fn tags_by_type(db: &Db, kind: DataType) -> plain_libs::rusqlite::Result<Vec<TagRow>> {
    db.with_conn(|c| {
        c.prepare(&format!(
            "SELECT {TAG_COLUMNS} FROM tags WHERE type=? ORDER BY rowid ASC"
        ))?
        .query_map([kind], row_to_tag)?
        .collect()
    })
}
pub fn tag_by_id(db: &Db, id: &str) -> plain_libs::rusqlite::Result<Option<TagRow>> {
    db.with_conn(|c| io::tag_by_id(c, id))
}
pub fn insert_tag(db: &Db, tag: &TagRow) -> plain_libs::rusqlite::Result<()> {
    db.with_conn(|c| {
        c.execute(
            "INSERT INTO tags (id,name,type,created_at,updated_at) VALUES (?1,?2,?3,?4,?4)",
            params![
                tag.id,
                tag.name,
                tag.kind,
                plain_libs::utils::dbtime::now_iso_millis()
            ],
        )?;
        Ok(())
    })
}
pub fn update_tag_name(db: &Db, id: &str, name: &str) -> plain_libs::rusqlite::Result<bool> {
    db.with_conn(|c| {
        Ok(c.execute(
            "UPDATE tags SET name=?1,updated_at=?2 WHERE id=?3",
            params![name, plain_libs::utils::dbtime::now_iso_millis(), id],
        )? > 0)
    })
}
pub fn rename_tag(db: &Db, id: &str, name: &str) -> plain_libs::rusqlite::Result<Option<TagRow>> {
    transaction(db, |c| {
        c.execute(
            "UPDATE tags SET name=?1,updated_at=?2 WHERE id=?3",
            params![name, plain_libs::utils::dbtime::now_iso_millis(), id],
        )?;
        io::tag_by_id(c, id)
    })
}
pub fn delete_tag(db: &Db, id: &str) -> plain_libs::rusqlite::Result<()> {
    db.with_conn(|c| {
        c.execute("DELETE FROM tags WHERE id=?", [id])?;
        Ok(())
    })
}
pub fn relations_for_key(db: &Db, key: &str) -> plain_libs::rusqlite::Result<Vec<TagRelationRow>> {
    db.with_conn(|c| {
        c.prepare(
            "SELECT tag_id,target_key FROM tag_relations WHERE target_key=? ORDER BY tag_id ASC",
        )?
        .query_map([key], row_to_relation)?
        .collect()
    })
}
pub fn relations_for_keys_of_kind(
    db: &Db,
    keys: &[String],
    kind: DataType,
) -> plain_libs::rusqlite::Result<Vec<TagRelationRow>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let keys = plain_libs::serde_json::to_string(keys)
        .map_err(|error| plain_libs::rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    db.with_conn(|c| {
        c.prepare("SELECT r.tag_id,r.target_key FROM json_each(?1) k
            JOIN tag_relations r ON r.target_key=k.value
            JOIN tags t ON t.id=r.tag_id
            WHERE t.type=?2 ORDER BY k.key,r.tag_id")?
            .query_map(params![keys,kind],row_to_relation)?
            .collect()
    })
}
pub fn tags_for_key_of_kind(db: &Db, key: &str, kind: DataType) -> plain_libs::rusqlite::Result<Vec<TagRow>> {
    db.with_conn(|c|c.prepare(&format!("SELECT {TAG_COLUMNS} FROM tags WHERE type=?1 AND id IN (SELECT tag_id FROM tag_relations WHERE target_key=?2) ORDER BY id"))?.query_map(params![kind,key],row_to_tag)?.collect())
}
pub fn keys_for_tag(db: &Db, id: &str) -> plain_libs::rusqlite::Result<Vec<String>> {
    db.with_conn(|c| {
        c.prepare("SELECT target_key FROM tag_relations WHERE tag_id=? ORDER BY rowid ASC")?
            .query_map([id], |r| r.get(0))?
            .collect()
    })
}
pub fn insert_relations(db: &Db, rels: &[(String, String)]) -> plain_libs::rusqlite::Result<()> {
    transaction(db, |c| io::insert_relations(c, rels))
}
pub fn remove_relations(db: &Db, keys: &[String], tag_ids: &[String]) -> plain_libs::rusqlite::Result<()> {
    transaction(db, |c| io::remove_relations(c, keys, tag_ids))
}
pub fn remove_relations_for_keys(db: &Db, keys: &[String]) -> plain_libs::rusqlite::Result<()> {
    transaction(db, |c| {
        c.execute(
            "DELETE FROM tag_relations WHERE target_key IN (SELECT value FROM json_each(?1))",
            [plain_libs::serde_json::to_string(keys)
                .map_err(|e| plain_libs::rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?],
        )?;
        Ok(())
    })
}

pub(crate) fn move_file_path(
    conn: &Connection,
    source: &str,
    destination: &str,
) -> plain_libs::rusqlite::Result<()> {
    if source == destination {
        return Ok(());
    }
    let kind = DataType::File;
    conn.execute("INSERT INTO tag_relations(tag_id,target_key,created_at) SELECT tag_id,?1,created_at FROM tag_relations WHERE EXISTS(SELECT 1 FROM tags WHERE id=tag_relations.tag_id AND type=?2) AND target_key=?3 ON CONFLICT(tag_id,target_key) DO UPDATE SET created_at=min(tag_relations.created_at,excluded.created_at)",params![destination,kind,source])?;
    conn.execute(
        "DELETE FROM tag_relations WHERE EXISTS(SELECT 1 FROM tags WHERE id=tag_relations.tag_id AND type=?1) AND target_key=?2",
        params![kind, source],
    )?;
    Ok(())
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/tag.rs"]
mod tests;
