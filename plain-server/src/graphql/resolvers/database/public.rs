//! Public `/graphql` database-browser roots — the web console's developer
//! view over the app's SQLite file.
//!

use async_graphql::{Context, Enum, Object, SimpleObject};
use plain_rs::db::devtools::SqliteFile;
use plain_rs::types::Long;
use std::sync::Arc;

#[derive(SimpleObject, Clone, Debug, Default)]
pub struct DbTableInfo {
    #[graphql(name = "idKey")]
    pub id_key: String,
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
    async fn db_path(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(app_db(ctx).await?.path().to_string_lossy().into_owned())
    }

    async fn db_tables(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<String>> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(vec![]);
        }
        spawn(move || Ok(file.tables()?.into_iter().collect())).await
    }

    async fn db_table_row_count(
        &self,
        ctx: &Context<'_>,
        table: String,
    ) -> async_graphql::Result<Long> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(Long(0));
        }
        spawn(move || file.table_row_count(&table).map(Long)).await
    }

    /// Rows come back as pre-rendered JSON strings, one per row — the
    /// column set differs per table, so there is no single row type to
    /// model.
    async fn db_table_rows(
        &self,
        ctx: &Context<'_>,
        table: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<String>> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(vec![]);
        }
        spawn(move || file.table_rows_text(&table, offset as i64, limit as i64)).await
    }

    async fn db_table_columns(
        &self,
        ctx: &Context<'_>,
        table: String,
    ) -> async_graphql::Result<Vec<DbTableColumn>> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(vec![]);
        }
        spawn(move || {
            Ok(file
                .table_columns(&table)?
                .into_iter()
                .map(|column| DbTableColumn {
                    name: column.name,
                    data_type: DbColumnType::parse(&column.data_type),
                    not_null: column.not_null,
                    default_value: column.default_value,
                    primary_key: column.primary_key,
                })
                .collect())
        })
        .await
    }

    async fn db_table_info(
        &self,
        ctx: &Context<'_>,
        table: String,
    ) -> async_graphql::Result<DbTableInfo> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(DbTableInfo {
                id_key: "id".to_string(),
            });
        }
        spawn(move || {
            Ok(DbTableInfo {
                id_key: file.table_id_key(&table)?,
            })
        })
        .await
    }
}

#[derive(Default)]
pub struct DbMutation;

#[Object]
impl DbMutation {
    /// `row` is a JSON object of column values. A malformed one is reported
    /// as `false` rather than an error: this is a developer console, and the
    /// operator is the one who typed it.
    async fn create_db_table_row(
        &self,
        ctx: &Context<'_>,
        table: String,
        row: String,
    ) -> async_graphql::Result<bool> {
        let file = app_db(ctx).await?;
        if !has_path(&file) {
            return Ok(false);
        }
        Ok(spawn(move || {
            let mut row: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&row)?;
            for value in row.values_mut() {
                if value.is_null() {
                    *value = serde_json::Value::String("null".into());
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
    ) -> async_graphql::Result<bool> {
        if ids.is_empty() {
            return Ok(false);
        }
        let file = app_db(ctx).await?;
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

async fn app_db(ctx: &Context<'_>) -> async_graphql::Result<SqliteFile> {
    let path = ctx
        .data::<Arc<plain_rs::db::Db>>()?
        .with_conn(|connection| connection.path().map(std::path::PathBuf::from))
        .unwrap_or_default();
    Ok(SqliteFile::new(path))
}

fn has_path(file: &SqliteFile) -> bool {
    !file.path().as_os_str().is_empty()
}

async fn spawn<T, F>(work: F) -> async_graphql::Result<T>
where
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?
        .map_err(|error| async_graphql::Error::new(error.to_string()))
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/database/public.rs"]
mod tests;
