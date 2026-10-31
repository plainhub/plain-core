use async_graphql::{Context, Result};
use plain_rs::services::settings::preferences::Preferences;
use plain_rs::services::settings::security_preferences::SecurityPreferences;
use plain_rs::services::settings::security_preferences::security_key;
use plain_rs::{events::WsEvent, prefs::Prefs};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;

fn prefs<'a>(ctx: &'a Context<'_>) -> Result<&'a Arc<Prefs>> {
    Ok(ctx.data::<Arc<Prefs>>()?)
}
fn events<'a>(ctx: &'a Context<'_>) -> Option<&'a broadcast::Sender<WsEvent>> {
    ctx.data_opt::<broadcast::Sender<WsEvent>>()
}
#[cfg(all(test, feature = "core"))]
#[path = "../../../../tests/unit/graphql/resolvers/settings/state.rs"]
mod tests;

#[cfg(all(test, feature = "core", feature = "http_transport"))]
#[path = "../../../../tests/unit/graphql/resolvers/settings/http.rs"]
mod http_tests;

pub(crate) fn preferences<'a>(ctx: &'a Context<'_>) -> Result<Preferences<'a>> {
    Ok(Preferences {
        prefs: prefs(ctx)?,
        events: events(ctx),
        #[cfg(feature = "core")]
        server: ctx.data_opt::<plain_rs::runtime::Runtime>(),
    })
}

pub(crate) async fn update(
    ctx: &Context<'_>,
    patch: plain_rs::prefs::settings::SettingsPatch,
) -> Result<bool> {
    let fields = serde_json::to_value(&patch)?;
    for (field, value) in fields.as_object().unwrap() {
        if value.is_null() {
            continue;
        }
        let key = match field.as_str() {
            "feedAutoRefreshIntervalSec" => "feed_auto_refresh_interval".to_owned(),
            "packageSortBy" => "pkg_sort_by".to_owned(),
            _ => field
                .chars()
                .flat_map(|c| {
                    if c.is_ascii_uppercase() {
                        vec!['_', c.to_ascii_lowercase()]
                    } else {
                        vec![c]
                    }
                })
                .collect(),
        };
        super::public::authorize(ctx, &key)?;
    }
    Ok(preferences(ctx)?.patch(patch).await?)
}

fn security_preferences<'a>(ctx: &'a Context<'_>) -> Result<SecurityPreferences<'a>> {
    Ok(SecurityPreferences {
        prefs: prefs(ctx)?,
        events: events(ctx),
        #[cfg(feature = "core")]
        server: ctx.data_opt::<plain_rs::runtime::Runtime>(),
    })
}

pub(crate) async fn set_pref(ctx: &Context<'_>, key: &str, value: Value) -> Result<bool> {
    super::public::authorize(ctx, key)?;
    Ok(preferences(ctx)?.set(key, value).await?)
}

pub(crate) async fn remove_pref(ctx: &Context<'_>, key: &str) -> Result<bool> {
    if security_key(key) {
        Ok(security_preferences(ctx)?.remove(key).await?)
    } else {
        Ok(preferences(ctx)?.remove(key).await?)
    }
}
