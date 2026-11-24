use plain_graphql::Context;

use plain_graphql::Json;

use super::authorize;
use plain_graphql::Object;
use plain_libs::serde_json::Value;
use plain_rs::features::prefs::pref_keys::{PrefAccess, RECENT_SELECTED_DIRS, access};

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
        if access(&key) == Some(PrefAccess::RustOnly) {
            return Err(plain_graphql::Error::new("Preference is not accessible"));
        }
        if key == RECENT_SELECTED_DIRS {
            authorize(ctx, &key).await?;
        }
        super::state::remove_pref(ctx, &key).await?;
        Ok(true)
    }
}
