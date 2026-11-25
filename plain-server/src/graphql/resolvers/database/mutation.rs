use plain_graphql::Context;

use crate::graphql::types::ActionResult;
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
        keys: Vec<Json<Map<String, Value>>>,
    ) -> plain_graphql::Result<ActionResult> {
        let file = app_db(ctx).await?;
        let keys = keys.into_iter().map(|key| key.0).collect::<Vec<_>>();
        spawn(move || {
            Ok(ActionResult {
                affected_count: file.delete_table_rows(&table, &keys)?.try_into()?,
            })
        })
        .await
    }
}
