use super::ScreenMirrorMode;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::Value;
use plain_rs::features::capability::Capability;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct ScreenMirrorMutation;

#[Object]
impl ScreenMirrorMutation {
    async fn start_screen_mirror(
        &self,
        ctx: &Context<'_>,
        audio: bool,
    ) -> plain_graphql::Result<bool> {
        dispatch(
            ctx,
            "systemStartScreenMirror",
            plain_libs::serde_json::json!({ "audio": audio }),
        )
        .await
    }

    async fn stop_screen_mirror(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        dispatch(
            ctx,
            "systemStopScreenMirror",
            plain_libs::serde_json::json!({}),
        )
        .await
    }

    /// Returns false when the device has not granted RECORD_AUDIO yet; the
    /// platform asks for it and the caller retries.
    async fn request_screen_mirror_audio(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::ScreenMirror,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let granted = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemRequestScreenMirrorAudio",
                plain_libs::serde_json::json!({}),
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        Ok(granted.as_bool().unwrap_or(false))
    }

    async fn request_screen_mirror_key_frame(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<bool> {
        dispatch(
            ctx,
            "systemRequestScreenMirrorKeyFrame",
            plain_libs::serde_json::json!({}),
        )
        .await
    }

    async fn update_screen_mirror_quality(
        &self,
        ctx: &Context<'_>,
        mode: ScreenMirrorMode,
    ) -> plain_graphql::Result<bool> {
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::ScreenMirror,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let quality = plain_libs::serde_json::json!({"mode":mode.name(),"resolution":if mode==ScreenMirrorMode::Smooth {720}else{1080}});
        crate::graphql::resolvers::settings::state::preferences(ctx)?
            .set(
                plain_rs::features::prefs::pref_keys::SCREEN_MIRROR_QUALITY,
                quality,
            )
            .await?;
        Ok(true)
    }
}

/// The two settings shortcuts the web console exposes. Both just raise the
/// matching app event, so the platform owns where the screen lands.
#[derive(Default)]
pub struct SettingsMutation;

#[Object]
impl SettingsMutation {
    async fn open_accessibility_settings(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        dispatch(
            ctx,
            "systemOpenAccessibilitySettings",
            plain_libs::serde_json::json!({}),
        )
        .await
    }
}

async fn dispatch(ctx: &Context<'_>, method: &str, params: Value) -> plain_graphql::Result<bool> {
    plain_rs::features::services::capabilities::require(
        ctx.data_unchecked::<Arc<Host>>(),
        Capability::ScreenMirror,
    )
    .await
    .map_err(|error| plain_graphql::Error::from(error))?;
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(|error| plain_graphql::Error::new(error))?;
    Ok(true)
}
