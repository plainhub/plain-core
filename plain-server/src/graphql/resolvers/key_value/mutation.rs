use super::validate_key;
use crate::server::ServerState;
use plain_graphql::{Context, Json, Object, Result};
use plain_libs::serde_json::Value;
use plain_rs::infrastructure::db::key_value_error::KeyValueErrorCode;

#[derive(Default)]
pub struct KeyValueMutation;

#[Object]
impl KeyValueMutation {
    /// Save JSON application data up to 65,536 encoded bytes; returns whether the stored value changed.
    fn set_key_value(&self, ctx: &Context<'_>, key: String, value: Json<Value>) -> Result<bool> {
        validate_key(&key)?;
        if value.0.to_string().len() > 65536 {
            return Err(plain_graphql::Error::from(
                KeyValueErrorCode::InvalidKeyValue,
            ));
        }
        let state = ctx.data::<ServerState>()?;
        Ok(state.store.get()?.set_json(&key, value.0)?)
    }
}
