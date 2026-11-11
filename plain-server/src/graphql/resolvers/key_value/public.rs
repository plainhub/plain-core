use crate::server::ServerState;
use plain_graphql::{Context, Json, Object, Result};
use plain_libs::serde_json::Value;

#[derive(Default)]
pub struct KeyValueQuery;

#[Object]
impl KeyValueQuery {
    /// Read JSON application data by key; returns null when absent and does not read preferences.
    fn key_value(&self, ctx: &Context<'_>, key: String) -> Result<Json<Value>> {
        validate_key(&key)?;
        let state = ctx.data::<ServerState>()?;
        Ok(Json(
            state.store.get()?.get_json(&key)?.unwrap_or(Value::Null),
        ))
    }
}

#[derive(Default)]
pub struct KeyValueMutation;

#[Object]
impl KeyValueMutation {
    /// Save JSON application data up to 65,536 encoded bytes; returns whether the stored value changed.
    fn set_key_value(&self, ctx: &Context<'_>, key: String, value: Json<Value>) -> Result<bool> {
        validate_key(&key)?;
        if value.0.to_string().len() > 65536 {
            return Err(plain_graphql::Error::new("invalid_key_value"));
        }
        let state = ctx.data::<ServerState>()?;
        Ok(state.store.get()?.set_json(&key, value.0)?)
    }
}

fn validate_key(key: &str) -> Result<()> {
    if matches!(
        key,
        "last_file_path" | "quick_note_draft" | "recent_searches" | "recent_save_dirs"
    ) || key.strip_prefix("chat_input_text:").is_some_and(|target| {
        (target.starts_with("channel:") || target.starts_with("peer:"))
            && target.split_once(':').is_some_and(|(_, id)| !id.is_empty())
    }) {
        Ok(())
    } else {
        Err(plain_graphql::Error::new("Unknown key-value key"))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/key_value/public.rs"]
mod tests;
