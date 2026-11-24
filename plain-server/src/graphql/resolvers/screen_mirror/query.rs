use super::{ScreenMirrorQuality, ScreenMirrorVideoCodec, codec, quality, state};
use plain_graphql::{Context, Object};
use plain_libs::serde_json::Value;
use plain_rs::features::capability::Capability;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

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
            Capability::ScreenMirror,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let stored = plain_rs::features::prefs::settings::snapshot(
            ctx.data_unchecked::<Arc<plain_rs::features::prefs::Prefs>>(),
        )?
        .screen_mirror_quality;
        Ok(quality(&stored))
    }
}
