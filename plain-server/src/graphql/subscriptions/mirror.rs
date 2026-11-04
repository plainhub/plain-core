use super::events;
use super::models::{MirrorCodecConfiguration, MirrorState};
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};

#[derive(Default)]
pub struct MirrorSubscriptions;
#[Subscription]
impl MirrorSubscriptions {
    async fn screen_mirror_state(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<MirrorState>>> {
        events::stream(ctx, "SCREEN_MIRRORING").await
    }
    async fn screen_mirror_video_codec(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Option<MirrorCodecConfiguration>>>> {
        events::stream(ctx, "SCREEN_MIRROR_VIDEO_CODEC").await
    }
    async fn screen_mirror_audio_granted(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "SCREEN_MIRROR_AUDIO_GRANTED").await
    }
}
