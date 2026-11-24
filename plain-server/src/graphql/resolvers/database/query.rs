use plain_graphql::Context;

use plain_graphql::Json;

use super::{DbColumnType, DbTable, DbTableColumn, app_db, has_path, spawn};
use plain_graphql::Object;
use plain_libs::serde_json::{Map, Value};
use plain_rs::infrastructure::types::Long;

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
