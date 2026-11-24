use super::super::query::prefs;
use super::PlaybackType;
use crate::http::auth::public_gate;
use plain_graphql::{Context, ID, Object, Result};
use plain_rs::features::media::play_history;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::types::Long;
use std::sync::Arc;

#[derive(Default)]
pub struct MediaPlayHistoryMutation;

#[Object]
impl MediaPlayHistoryMutation {
    async fn record_media_play(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: ID,
        r#type: PlaybackType,
        position_ms: Long,
    ) -> Result<bool> {
        public_gate::require(
            ctx,
            prefs(ctx),
            &[super::super::query::access_feature(r#type.data_type())],
        )?;
        let db = ctx.data::<Arc<Db>>()?.clone();
        tokio::task::spawn_blocking(move || {
            play_history::record(&db, r#type.data_type(), file_id.as_str(), position_ms.0)
        })
        .await??;
        if r#type == PlaybackType::Audio {
            plain_rs::features::services::audio::reports::publish_library_updated(
                ctx.data::<Arc<plain_rs::infrastructure::host::bridge::Host>>()?,
            );
        }
        Ok(true)
    }

    async fn save_media_play_progress(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: ID,
        r#type: PlaybackType,
        position_ms: Long,
    ) -> Result<bool> {
        public_gate::require(
            ctx,
            prefs(ctx),
            &[super::super::query::access_feature(r#type.data_type())],
        )?;
        let db = ctx.data::<Arc<Db>>()?.clone();
        tokio::task::spawn_blocking(move || {
            play_history::save(&db, r#type.data_type(), file_id.as_str(), position_ms.0)
        })
        .await??;
        Ok(true)
    }
}
