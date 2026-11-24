use super::validate_key;
use crate::server::ServerState;
use plain_graphql::{Context, Json, Object, Result};
use plain_libs::serde_json::Value;

#[derive(Default)]
pub struct KeyValueQuery;

#[Object]
impl KeyValueQuery {
    /// Read JSON application data by key; returns null when absent and does not read preferences.
    fn key_value(&self, ctx: &Context<'_>, key: String) -> Result<Option<Json<Value>>> {
        validate_key(&key)?;
        let state = ctx.data::<ServerState>()?;
        Ok(state.store.get()?.get_json(&key)?.map(Json))
    }
}
