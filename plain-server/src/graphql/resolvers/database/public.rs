//! Public `/graphql` database-browser roots — the web console's developer
//! view over the app's SQLite file.
//!

use plain_graphql::{Context, Enum, Json, Object, SimpleObject};
use plain_libs::serde_json::{Map, Value};

use plain_rs::infrastructure::db::devtools::SqliteFile;
use plain_rs::infrastructure::types::Long;
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

#[derive(Default)]
pub struct DbQuery;

#[Object]
impl DbQuery {
    async fn db_path(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        Ok(app_db(ctx).await?.path().to_string_lossy().into_owned())
    }

    async fn db_tables(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<DbTable>> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(vec![]);
        }
        spawn(move || {
            Ok(file
                .table_schemas()?
                .into_iter()
                .map(|table| DbTable {
                    name: table.name,
                    create_sql: table.create_sql,
                    columns: table
                        .columns
                        .into_iter()
                        .map(|column| DbTableColumn {
                            name: column.name,
                            data_type: DbColumnType::parse(&column.data_type),
                            not_null: column.not_null,
                            default_value: column.default_value,
                            primary_key: column.primary_key,
                        })
                        .collect(),
                })
                .collect())
        })
        .await
    }

    async fn db_table_row_count(
        &self,
        ctx: &Context<'_>,
        table: String,
    ) -> plain_graphql::Result<Long> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(Long(0));
        }
        spawn(move || file.table_row_count(&table).map(Long)).await
    }

    async fn db_table_rows(
        &self,
        ctx: &Context<'_>,
        table: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<Json<Map<String, Value>>>> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(vec![]);
        }
        spawn(move || file.table_rows_text(&table, offset as i64, limit as i64))
            .await
            .map(|rows| rows.into_iter().map(Json).collect())
    }
}

#[derive(Default)]
pub struct DbMutation;

#[Object]
impl DbMutation {
    async fn create_db_table_row(
        &self,
        ctx: &Context<'_>,
        table: String,
        row: Json<Map<String, Value>>,
    ) -> plain_graphql::Result<bool> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(false);
        }
        Ok(spawn(move || {
            let mut row = row.0;
            for value in row.values_mut() {
                if value.is_null() {
                    *value = plain_libs::serde_json::Value::String("null".into());
                }
            }
            file.insert_table_row(&table, &row).map(|_| true)
        })
        .await
        .unwrap_or(false))
    }

    async fn delete_db_table_rows(
        &self,
        ctx: &Context<'_>,
        table: String,
        ids: Vec<String>,
    ) -> plain_graphql::Result<bool> {
        let file = app_db(ctx).await?;
        if ids.is_empty() {
            return Ok(false);
        }
        if !has_path(&file) {
            return Ok(false);
        }
        Ok(
            spawn(move || file.delete_table_rows(&table, &ids).map(|_| true))
                .await
                .unwrap_or(false),
        )
    }
}

async fn app_db(ctx: &Context<'_>) -> plain_graphql::Result<SqliteFile> {
    super::super::common::require_developer_mode(ctx)?;
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
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?
        .map_err(|error| plain_graphql::Error::new(error.to_string()))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/database/public.rs"]
mod tests;
