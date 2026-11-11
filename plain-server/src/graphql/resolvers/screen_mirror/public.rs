//! Public `/graphql` screen-mirror roots.
//!
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::text;
use plain_graphql::{Context, Enum, Object, SimpleObject};

use plain_rs::infrastructure::host::bridge::Host;
use plain_libs::serde_json::Value;
use std::sync::Arc;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum ScreenMirrorMode {
    #[default]
    Hd,
    Smooth,
}

impl ScreenMirrorMode {
    fn parse(value: &str) -> Self {
        match value {
            "SMOOTH" => Self::Smooth,
            _ => Self::Hd,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Hd => "HD",
            Self::Smooth => "SMOOTH",
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ScreenMirrorQuality {
    pub mode: ScreenMirrorMode,
    pub resolution: i32,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ScreenMirrorVideoCodec {
    /// H.264 Annex-B codec configuration (SPS/PPS), base64-encoded.
    #[graphql(name = "annexB")]
    pub annex_b: String,
    /// Latest H.264 keyframe, base64-encoded; null until the pipeline emits one.
    #[graphql(name = "keyFrame")]
    pub key_frame: Option<String>,
}

#[derive(Default)]
pub struct ScreenMirrorQuery;

#[Object]
impl ScreenMirrorQuery {
    async fn is_screen_mirroring(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        Ok(state(ctx).await?["running"].as_bool().unwrap_or(false))
    }

    async fn screen_mirror_control_enabled(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<bool> {
        Ok(state(ctx).await?["controlEnabled"]
            .as_bool()
            .unwrap_or(false))
    }

    /// Null while the pipeline has not published its codec configuration.
    async fn screen_mirror_video_codec(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<Option<ScreenMirrorVideoCodec>> {
        let state = state(ctx).await?;
        let Some(published) = state["codec"].as_object() else {
            return Ok(None);
        };
        let published = Value::Object(published.clone());
        Ok(Some(codec(&published)))
    }

    async fn screen_mirror_quality(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<ScreenMirrorQuality> {
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            "SCREEN_MIRROR",
        )
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        let stored = plain_rs::features::prefs::settings::snapshot(
            ctx.data_unchecked::<Arc<plain_rs::features::prefs::Prefs>>(),
        )?
        .screen_mirror_quality;
        Ok(quality(&stored))
    }
}

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
        dispatch(ctx, "systemStopScreenMirror", plain_libs::serde_json::json!({})).await
    }

    /// Returns false when the device has not granted RECORD_AUDIO yet; the
    /// platform asks for it and the caller retries.
    async fn request_screen_mirror_audio(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            "SCREEN_MIRROR",
        )
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        let granted = ctx
            .data_unchecked::<Arc<Host>>()
            .call("systemRequestScreenMirrorAudio", plain_libs::serde_json::json!({}))
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
            "SCREEN_MIRROR",
        )
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
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

async fn state(ctx: &Context<'_>) -> plain_graphql::Result<Value> {
    plain_rs::features::services::capabilities::require(
        ctx.data_unchecked::<Arc<Host>>(),
        "SCREEN_MIRROR",
    )
    .await
    .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
    ctx.data_unchecked::<Arc<Host>>()
        .call("systemScreenMirrorState", plain_libs::serde_json::json!({}))
        .await
        .map_err(|error| plain_graphql::Error::new(error))
}

async fn dispatch(ctx: &Context<'_>, method: &str, params: Value) -> plain_graphql::Result<bool> {
    plain_rs::features::services::capabilities::require(
        ctx.data_unchecked::<Arc<Host>>(),
        "SCREEN_MIRROR",
    )
    .await
    .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(|error| plain_graphql::Error::new(error))?;
    Ok(true)
}

fn codec(value: &Value) -> ScreenMirrorVideoCodec {
    ScreenMirrorVideoCodec {
        annex_b: text(value, "annexB"),
        key_frame: value["keyFrame"]
            .as_str()
            .filter(|key_frame| !key_frame.is_empty())
            .map(str::to_owned),
    }
}

fn quality(value: &Value) -> ScreenMirrorQuality {
    ScreenMirrorQuality {
        mode: ScreenMirrorMode::parse(&text(value, "mode")),
        resolution: integer(value, "resolution") as i32,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/screen_mirror/public.rs"]
mod tests;
