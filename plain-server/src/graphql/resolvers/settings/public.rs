//! Public preference roots share the local Rust store and notify UI projections.

use plain_graphql::{Context, Json, Object};

use plain_rs::features::prefs::Prefs;
use plain_rs::features::prefs::pref_keys::{APP_HOME_FEATURES, AUTO_CHECK_UPDATE, SKIP_VERSION};
use serde_json::Value;
use std::sync::Arc;

#[derive(Default)]
pub struct PrefsQuery;

#[Object]
impl PrefsQuery {
    fn prefs(
        &self,
        ctx: &Context<'_>,
        keys: Option<Vec<String>>,
    ) -> plain_graphql::Result<Json<Value>> {
        let prefs = prefs(ctx);
        let Some(keys) = keys else {
            super::super::common::require_developer_mode(ctx)?;
            return Ok(Json(Value::Object(prefs.entries().into_iter().collect())));
        };
        let mut values = serde_json::Map::new();
        for key in keys {
            if !plain_rs::features::prefs::pref_keys::readable(&key, is_local(ctx)) {
                return Err(plain_graphql::Error::new("Preference is not accessible"));
            }
            if let Some(value) = prefs.get::<Value>(&key)? {
                let value =
                    plain_rs::features::prefs::settings::get_entry(prefs, &key)?.unwrap_or(value);
                values.insert(key, value);
            } else if key == APP_HOME_FEATURES {
                values.insert(
                    key,
                    Value::String(
                        plain_rs::features::prefs::settings::Settings::default().app_home_features,
                    ),
                );
            }
        }
        Ok(Json(Value::Object(values)))
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
    ) -> plain_graphql::Result<bool> {
        super::state::set_pref(ctx, &key, value.0).await?;
        Ok(true)
    }

    async fn remove_pref(&self, ctx: &Context<'_>, key: String) -> plain_graphql::Result<bool> {
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
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<Prefs> {
    ctx.data_unchecked::<Arc<Prefs>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/settings/public.rs"]
mod tests;

fn is_local(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        == Some(&crate::graphql::executor::RequestOrigin::Local)
}
pub(crate) fn allowed(ctx: &Context<'_>, key: &str) -> bool {
    plain_rs::features::prefs::pref_keys::writable(key, is_local(ctx))
}
pub(crate) fn authorize(ctx: &Context<'_>, key: &str) -> plain_graphql::Result<()> {
    if !allowed(ctx, key) {
        return Err(plain_graphql::Error::new("Preference is not accessible"));
    }
    Ok(())
}
