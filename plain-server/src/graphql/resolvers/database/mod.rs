//! Public `/graphql` database-browser roots — the web console's developer
//! view over the app's SQLite file.
//!

pub mod mutation;
pub mod query;

use plain_graphql::Context;

use plain_graphql::Enum;

use plain_graphql::SimpleObject;
use plain_rs::infrastructure::db::devtools::SqliteFile;
use std::sync::Arc;

#[derive(SimpleObject, Clone, Debug, Default)]
pub struct DbTable {
    pub name: String,
    pub create_sql: String,
    pub columns: Vec<DbTableColumn>,
}

#[derive(SimpleObject, Clone, Debug, Default)]
pub struct DbTableColumn {
    pub name: String,
    #[graphql(name = "dataType")]
    pub data_type: DbColumnType,
    #[graphql(name = "notNull")]
    pub not_null: bool,
    #[graphql(name = "defaultValue")]
    pub default_value: Option<String>,
    #[graphql(name = "primaryKey")]
    pub primary_key: bool,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum DbColumnType {
    Text,
    Integer,
    Real,
    Blob,
    Numeric,
    #[default]
    Unknown,
}

impl DbColumnType {
    fn parse(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "TEXT" => Self::Text,
            "INTEGER" | "INT" => Self::Integer,
            "REAL" => Self::Real,
            "BLOB" => Self::Blob,
            "NUMERIC" => Self::Numeric,
            _ => Self::Unknown,
        }
    }
}

async fn app_db(ctx: &Context<'_>) -> plain_graphql::Result<SqliteFile> {
    super::common::require_developer_mode(ctx)?;
    let path = ctx
        .data::<Arc<plain_rs::infrastructure::db::Db>>()?
        .with_conn(|connection| connection.path().map(std::path::PathBuf::from))
        .unwrap_or_default();
    Ok(SqliteFile::system(path))
}

fn has_path(file: &SqliteFile) -> bool {
    !file.path().as_os_str().is_empty()
}

async fn spawn<T, F>(work: F) -> plain_graphql::Result<T>
where
    F: FnOnce() -> plain_libs::anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| plain_graphql::Error::from(error))?
        .map_err(|error| plain_graphql::Error::from(error))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/database/mod.rs"]
mod tests;
