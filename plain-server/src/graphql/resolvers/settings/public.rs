//! Public preference roots share the local Rust store and notify UI projections.

use plain_graphql::{Context, Json, Object};

use plain_libs::serde_json::Value;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::prefs::pref_keys::{
    APP_HOME_FEATURES, PrefAccess, RECENT_SELECTED_DIRS, access,
};
use std::sync::Arc;

#[derive(Default)]
pub struct PrefsQuery;

#[Object]
impl PrefsQuery {
    async fn prefs(
        &self,
        ctx: &Context<'_>,
        keys: Option<Vec<String>>,
    ) -> plain_graphql::Result<Json<Value>> {
        let prefs = prefs(ctx);
        let Some(keys) = keys else {
            super::super::common::require_developer_mode(ctx)?;
            let can_read_recent_dirs = files_preference_allowed(ctx);
            return Ok(Json(Value::Object(
                prefs
                    .entries()
                    .into_iter()
                    .filter(|(key, _)| key != RECENT_SELECTED_DIRS || can_read_recent_dirs)
                    .collect(),
            )));
        };
        let mut values = plain_libs::serde_json::Map::new();
        for key in keys {
            if !readable(ctx, &key).await? {
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
        super::super::common::require_developer_mode(ctx)?;
        if key == RECENT_SELECTED_DIRS {
            authorize(ctx, &key).await?;
        }
        super::state::remove_pref(ctx, &key).await?;
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
pub(crate) async fn authorize(ctx: &Context<'_>, key: &str) -> plain_graphql::Result<()> {
    let allowed = if access(key) == Some(PrefAccess::LocalOrFiles) {
        files_preference_allowed(ctx)
    } else {
        let capabilities = preference_capabilities(ctx, key).await?;
        plain_rs::features::prefs::pref_keys::writable(key, is_local(ctx), &capabilities)
    };
    if !allowed {
        return Err(plain_graphql::Error::new("Preference is not accessible"));
    }
    Ok(())
}

async fn readable(ctx: &Context<'_>, key: &str) -> plain_graphql::Result<bool> {
    if access(key) == Some(PrefAccess::LocalOrFiles) {
        return Ok(files_preference_allowed(ctx));
    }
    let capabilities = preference_capabilities(ctx, key).await?;
    Ok(plain_rs::features::prefs::pref_keys::readable(
        key,
        is_local(ctx),
        &capabilities,
    ))
}

async fn preference_capabilities(
    ctx: &Context<'_>,
    key: &str,
) -> plain_graphql::Result<Vec<plain_rs::features::capability::Capability>> {
    if is_local(ctx) || access(key) != Some(PrefAccess::LocalOrHttpServer) {
        return Ok(Vec::new());
    }
    crate::graphql::resolvers::app::http_server::authorized_capabilities(ctx).await
}

fn files_preference_allowed(ctx: &Context<'_>) -> bool {
    use crate::graphql::executor::RequestClient;
    use plain_rs::features::access::AccessFeature;
    is_local(ctx)
        || (ctx.data_opt::<RequestClient>().is_some()
            && crate::http::auth::public_gate::require(ctx, prefs(ctx), &[AccessFeature::Files])
                .is_ok())
}
