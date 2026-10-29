use crate::db::Db;
use crate::sqlite_browse as browse;
use crate::sqlite_browse::TableColumnMeta;
use crate::sqlite_browse::rusqlite::{Connection, OpenFlags};
use anyhow::{Result, bail};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn browse_err(e: browse::SqliteBrowseError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn checked(db: &Db, table: &str) -> Result<()> {
    if db.with_conn(|conn| browse::table_exists(conn, table)) {
        Ok(())
    } else {
        bail!("Table not found: {table}")
    }
}

pub fn tables(db: &Db) -> Vec<String> {
    db.with_conn(|conn| browse::tables(conn))
}

pub fn table_row_count(db: &Db, table: &str) -> Result<i64> {
    checked(db, table)?;
    db.with_conn(|conn| browse::row_count(conn, table))
        .map_err(browse_err)
}

pub fn table_rows(db: &Db, table: &str, offset: i64, limit: i64) -> Result<Vec<String>> {
    checked(db, table)?;
    db.with_conn(|conn| browse::rows_page(conn, table, offset, limit))
        .map_err(browse_err)
}

pub fn table_id_key(db: &Db, table: &str) -> Result<String> {
    checked(db, table)?;
    Ok(db.with_conn(|conn| {
        browse::primary_key_column(conn, table).unwrap_or_else(|| "rowid".to_string())
    }))
}

pub fn table_columns(db: &Db, table: &str) -> Result<Vec<TableColumnMeta>> {
    checked(db, table)?;
    Ok(db.with_conn(|conn| browse::table_columns(conn, table)))
}

pub fn delete_table_rows(db: &Db, table: &str, ids: &[String]) -> Result<usize> {
    checked(db, table)?;
    db.with_conn(|conn| {
        let key = browse::primary_key_column(conn, table).unwrap_or_else(|| "rowid".to_string());
        browse::delete_rows(conn, table, &key, ids)
    })
    .map_err(browse_err)
}

pub struct SqliteFile {
    path: PathBuf,
}

impl SqliteFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn tables(&self) -> Result<Vec<String>> {
        Ok(browse::tables(&self.open()?))
    }

    pub fn table_row_count(&self, table: &str) -> Result<i64> {
        let conn = self.open()?;
        self.checked(&conn, table)?;
        browse::row_count(&conn, table).map_err(browse_err)
    }

    pub fn table_rows_text(&self, table: &str, offset: i64, limit: i64) -> Result<Vec<String>> {
        let conn = self.open()?;
        self.checked(&conn, table)?;
        browse::rows_page_text(&conn, table, offset, limit).map_err(browse_err)
    }

    pub fn table_columns(&self, table: &str) -> Result<Vec<TableColumnMeta>> {
        let conn = self.open()?;
        self.checked(&conn, table)?;
        Ok(browse::table_columns(&conn, table))
    }

    pub fn table_id_key(&self, table: &str) -> Result<String> {
        let conn = self.open()?;
        self.checked(&conn, table)?;
        Ok(self.id_key(&conn, table))
    }

    pub fn insert_table_row(&self, table: &str, row: &Map<String, Value>) -> Result<usize> {
        let conn = self.open()?;
        self.checked(&conn, table)?;
        browse::insert_row(&conn, table, row).map_err(browse_err)
    }

    pub fn delete_table_rows(&self, table: &str, ids: &[String]) -> Result<usize> {
        let conn = self.open()?;
        self.checked(&conn, table)?;
        browse::delete_rows(&conn, table, &self.id_key(&conn, table), ids).map_err(browse_err)
    }

    fn open(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        Ok(conn)
    }

    fn checked(&self, conn: &Connection, table: &str) -> Result<()> {
        if browse::table_exists(conn, table) {
            Ok(())
        } else {
            bail!("Table not found or invalid identifier: {table}")
        }
    }

    fn id_key(&self, conn: &Connection, table: &str) -> String {
        browse::primary_key_column(conn, table).unwrap_or_else(|| "id".to_string())
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/devtools.rs"]
mod tests;
