use plain_graphql::Context;

use plain_graphql::Json;

use super::{files_preference_allowed, is_local, preference_capabilities, prefs};
use plain_graphql::Object;
use plain_libs::serde_json::Value;
use plain_rs::features::prefs::pref_keys::{
    APP_HOME_FEATURES, PrefAccess, RECENT_SELECTED_DIRS, access,
};

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
