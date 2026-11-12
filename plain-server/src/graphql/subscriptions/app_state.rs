use super::{events, models::*};
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};
#[derive(Default)]
pub struct AppStateSubscriptions;
#[Subscription]
impl AppStateSubscriptions {
    async fn prefs_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "PREFS_UPDATED").await
    }
    async fn peers_changed(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "PEER_UPDATED").await
    }
    async fn channels_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<ChannelChange>>> {
        events::stream(ctx, "CHANNELS_UPDATED").await
    }
    async fn mdns_changed(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "MDNS_UPDATED").await
    }
    async fn nearby_devices_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "NEARBY_DEVICES_UPDATED").await
    }
    async fn downloads_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "DOWNLOAD_PROGRESS").await
    }
    async fn image_models_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<ImageModelsUpdate>>> {
        events::stream(ctx, "IMAGE_MODELS_UPDATED").await
    }
    async fn pairing_changed(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "PAIRING_STATE_CHANGED").await
    }
    async fn pairing_requested(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<PairingRequestNotice>>> {
        events::stream(ctx, "PAIRING_REQUEST_RECEIVED").await
    }
    async fn pairing_completed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<PairingCompletedNotice>>> {
        events::stream(ctx, "PAIRING_SUCCESS").await
    }
    async fn pairing_started(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<PairingProgressNotice>>> {
        events::stream(ctx, "PAIRING_STARTED").await
    }
    async fn pairing_failed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<PairingProgressNotice>>> {
        events::stream(ctx, "PAIRING_FAILED").await
    }
    async fn pairing_canceled(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<PairingProgressNotice>>> {
        events::stream(ctx, "PAIRING_CANCELED").await
    }
    async fn pomodoro_settings_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<bool>>> {
        events::stream(ctx, "POMODORO_SETTINGS_UPDATE").await
    }
    async fn pomodoro_action(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<PomodoroAction>>> {
        events::stream(ctx, "POMODORO_ACTION").await
    }
}
