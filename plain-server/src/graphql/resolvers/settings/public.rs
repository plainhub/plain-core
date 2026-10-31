//! Public preference roots share the local Rust store and notify UI projections.

use async_graphql::{Context, Json, Object};
use plain_rs::prefs::Prefs;
use plain_rs::prefs::pref_keys::{APP_HOME_FEATURES, AUTO_CHECK_UPDATE, SKIP_VERSION};
use serde_json::Value;
use std::sync::Arc;

/// Measured in bytes, not characters: the limit is what the store has to
/// hold, and a multi-byte value would slip past a character count.
fn validate_value(value: &Value) -> async_graphql::Result<()> {
    if value.to_string().len() > 65536 {
        return Err(async_graphql::Error::new("invalid_pref_value"));
    }
    Ok(())
}

#[derive(Default)]
pub struct PrefsQuery;

#[Object]
impl PrefsQuery {
    async fn prefs(&self, ctx: &Context<'_>) -> async_graphql::Result<Json<Value>> {
        let prefs = prefs(ctx);
        let mut values = serde_json::Map::new();
        for (key, value) in prefs.entries() {
            if plain_rs::prefs::pref_keys::readable(&key, is_local(ctx)) {
                let value = plain_rs::prefs::settings::get_entry(prefs, &key)?.unwrap_or(value);
                values.insert(key, value);
            }
        }
        values
            .entry(APP_HOME_FEATURES.to_owned())
            .or_insert_with(|| {
                Value::String(plain_rs::prefs::settings::Settings::default().app_home_features)
            });
        Ok(Json(Value::Object(values)))
    }
    async fn saved_value(
        &self,
        ctx: &Context<'_>,
        key: String,
    ) -> async_graphql::Result<Json<Value>> {
        saved_key(&key)?;
        let state = ctx.data::<crate::server::ServerState>()?;
        Ok(Json(
            state.store.get()?.get_json(&key)?.unwrap_or(Value::Null),
        ))
    }
}

#[derive(Default)]
pub struct PrefsMutation;

#[Object]
impl PrefsMutation {
    async fn set_pref(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: Json<Value>,
    ) -> async_graphql::Result<bool> {
        super::state::set_pref(ctx, &key, value.0).await?;
        Ok(true)
    }

    async fn remove_pref(&self, ctx: &Context<'_>, key: String) -> async_graphql::Result<bool> {
        authorize(ctx, &key)?;
        if matches!(key.as_str(), SKIP_VERSION | AUTO_CHECK_UPDATE) {
            let default = if key == SKIP_VERSION {
                Value::String(String::new())
            } else {
                Value::Bool(true)
            };
            super::state::set_pref(ctx, &key, default).await?;
        } else {
            super::state::remove_pref(ctx, &key).await?;
        }
        Ok(true)
    }
    async fn set_saved_value(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: Json<Value>,
    ) -> async_graphql::Result<bool> {
        saved_key(&key)?;
        validate_value(&value.0)?;
        let state = ctx.data::<crate::server::ServerState>()?;
        Ok(state.store.get()?.set_json(&key, value.0)?)
    }
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<Prefs> {
    ctx.data_unchecked::<Arc<Prefs>>()
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/settings/public.rs"]
mod tests;

fn is_local(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        == Some(&crate::graphql::executor::RequestOrigin::Local)
}
pub(crate) fn allowed(ctx: &Context<'_>, key: &str) -> bool {
    plain_rs::prefs::pref_keys::writable(key, is_local(ctx))
}
pub(crate) fn authorize(ctx: &Context<'_>, key: &str) -> async_graphql::Result<()> {
    if !allowed(ctx, key) {
        return Err(async_graphql::Error::new("Preference is not accessible"));
    }
    Ok(())
}

fn saved_key(key: &str) -> async_graphql::Result<()> {
    if matches!(
        key,
        "last_file_path" | "quick_note_draft" | "recent_searches" | "recent_save_dirs"
    ) || key.strip_prefix("chat_input_text:").is_some_and(|target| {
        (target.starts_with("channel:") || target.starts_with("peer:"))
            && target.split_once(':').is_some_and(|(_, id)| !id.is_empty())
    }) {
        Ok(())
    } else {
        Err(async_graphql::Error::new("Unknown saved data key"))
    }
}
