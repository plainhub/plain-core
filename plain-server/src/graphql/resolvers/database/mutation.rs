use plain_graphql::Context;

use plain_graphql::Json;

use super::{app_db, has_path, spawn};
use plain_graphql::Object;
use plain_libs::serde_json::{Map, Value};

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
        spawn(move || file.insert_table_row(&table, &row.0).map(|_| true)).await
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
        spawn(move || file.delete_table_rows(&table, &ids).map(|_| true)).await
    }
}
