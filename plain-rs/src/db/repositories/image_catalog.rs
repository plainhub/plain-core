use crate::db::Db;
use crate::error::{DomainError, DomainResult};
use crate::media::images::indexing::{Image, Page, Snapshot};
use rusqlite::{Connection, params, params_from_iter};

pub(crate) fn init(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TEMP TABLE image_catalog_revision (revision INTEGER NOT NULL);
        INSERT INTO image_catalog_revision VALUES(0);
        CREATE TEMP TRIGGER image_catalog_insert AFTER INSERT ON main.files WHEN NEW.kind='IMAGE' BEGIN
            UPDATE image_catalog_revision SET revision=revision+1;
        END;
        CREATE TEMP TRIGGER image_catalog_delete AFTER DELETE ON main.files WHEN OLD.kind='IMAGE' BEGIN
            UPDATE image_catalog_revision SET revision=revision+1;
        END;
        CREATE TEMP TRIGGER image_catalog_update AFTER UPDATE ON main.files
        WHEN (OLD.kind='IMAGE' OR NEW.kind='IMAGE') AND (
            OLD.id IS NOT NEW.id OR OLD.path IS NOT NEW.path OR OLD.kind IS NOT NEW.kind OR
            OLD.trashed_at IS NOT NEW.trashed_at OR OLD.missing IS NOT NEW.missing OR OLD.volume_id IS NOT NEW.volume_id OR OLD.size_bytes IS NOT NEW.size_bytes OR OLD.updated_at IS NOT NEW.updated_at
        ) BEGIN
            UPDATE image_catalog_revision SET revision=revision+1;
        END;
        CREATE TEMP TRIGGER image_catalog_volume_update AFTER UPDATE OF path,is_available ON main.volumes
        WHEN OLD.path IS NOT NEW.path OR OLD.is_available IS NOT NEW.is_available BEGIN
            UPDATE image_catalog_revision SET revision=revision+1;
        END;"
    )
}

fn revision(connection: &Connection) -> DomainResult<String> {
    let external: i64 = connection.pragma_query_value(None, "data_version", |row| row.get(0))?;
    let local: i64 = connection.query_row(
        "SELECT revision FROM temp.image_catalog_revision",
        [],
        |row| row.get(0),
    )?;
    Ok(format!("{local}:{external}"))
}

fn read<T>(
    db: &Db,
    expected: &str,
    operation: impl FnOnce(&Connection) -> DomainResult<T>,
) -> DomainResult<T> {
    db.with_conn(|connection| {
        if revision(connection)? != expected {
            return Err(DomainError::Other(
                "Image catalog changed during indexing".into(),
            ));
        }
        let value = operation(connection)?;
        if revision(connection)? != expected {
            return Err(DomainError::Other(
                "Image catalog changed during indexing".into(),
            ));
        }
        Ok(value)
    })
}

pub fn snapshot(db: &Db) -> DomainResult<Snapshot> {
    db.with_conn(|connection| {
        let before = revision(connection)?;
        let total: usize = connection.query_row(
            "SELECT count(*) FROM files f JOIN volumes v ON v.id=f.volume_id WHERE f.kind='IMAGE' AND f.trashed_at IS NULL AND f.missing=0 AND v.is_available=1",
            [],
            |row| crate::db::read_count(row, 0),
        )?;
        if revision(connection)? != before {
            return Err(DomainError::Other(
                "Image catalog changed during indexing".into(),
            ));
        }
        Ok(Snapshot {
            revision: before,
            total,
        })
    })
}

pub fn page(db: &Db, expected: &str, cursor: &str, limit: usize) -> DomainResult<Page> {
    if !(1..=128).contains(&limit) {
        return Err(DomainError::Other("Invalid image catalog page size".into()));
    }
    read(db, expected, |connection| {
        let mut statement = connection.prepare("SELECT CAST(f.id AS TEXT),rtrim(v.path,'/')||'/'||f.path FROM files f JOIN volumes v ON v.id=f.volume_id WHERE f.kind='IMAGE' AND f.trashed_at IS NULL AND f.missing=0 AND v.is_available=1 AND f.id>CAST(?1 AS INTEGER) ORDER BY f.id LIMIT ?2")?;
        let items = statement
            .query_map(params![cursor, limit as i64], |row| {
                Ok(Image {
                    id: row.get(0)?,
                    path: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Page {
            revision: expected.to_owned(),
            next_cursor: items
                .last()
                .map_or_else(|| cursor.to_owned(), |item| item.id.clone()),
            done: items.len() < limit,
            items,
        })
    })
}

pub fn resolve(db: &Db, expected: &str, ids: &[String]) -> DomainResult<Vec<Image>> {
    if ids.len() > 100 || ids.iter().any(String::is_empty) {
        return Err(DomainError::Other("Invalid image catalog selection".into()));
    }
    read(db, expected, |connection| {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT CAST(f.id AS TEXT),rtrim(v.path,'/')||'/'||f.path FROM files f JOIN volumes v ON v.id=f.volume_id WHERE f.kind='IMAGE' AND f.trashed_at IS NULL AND f.missing=0 AND v.is_available=1 AND f.id IN ({}) ORDER BY f.id",
            vec!["?"; ids.len()].join(",")
        );
        let mut statement = connection.prepare(&sql)?;
        Ok(statement
            .query_map(params_from_iter(ids), |row| {
                Ok(Image {
                    id: row.get(0)?,
                    path: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

pub fn verify(db: &Db, expected: &str) -> DomainResult<()> {
    read(db, expected, |_| Ok(()))
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/image_catalog.rs"]
mod tests;
