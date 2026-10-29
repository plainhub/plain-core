use crate::graphql::resolvers::audio::public::AudioItem;
use crate::graphql::types::Long;
use crate::services::audio::host::Audio;
use async_graphql::{Context, ID, Object, Result, SimpleObject};
use plain_rs::media::audio::{commands, playback, queue};
use plain_rs::prefs::Prefs;
use std::sync::Arc;

#[derive(SimpleObject)]
pub struct AudioPlaybackState {
    path: String,
    position_ms: Long,
    revision: Long,
}
impl From<playback::Playback> for AudioPlaybackState {
    fn from(value: playback::Playback) -> Self {
        Self {
            path: value.path,
            position_ms: Long(value.position_ms),
            revision: Long(value.revision),
        }
    }
}

#[derive(SimpleObject)]
pub struct AudioSelection {
    track: Option<AudioItem>,
    path: String,
    position_ms: Long,
    revision: Long,
}

#[derive(Default)]
pub struct AudioCommandsMutation;

#[Object]
impl AudioCommandsMutation {
    async fn advance_audio_selection(
        &self,
        ctx: &Context<'_>,
        action: plain_rs::media::audio::selection::AudioSelectionAction,
        expected_path: String,
        expected_revision: Long,
        #[graphql(default)] shuffle: bool,
    ) -> Result<Option<AudioSelection>> {
        if expected_revision.0 < 0 {
            return Err(async_graphql::Error::new(
                "invalid audio selection revision",
            ));
        }
        let prefs = ctx.data::<Arc<Prefs>>()?.clone();
        let selection = super::public::audio(ctx)?
            .run_playback(move |db, engine| {
                use commands::Engine;
                let current = playback::snapshot(db)?;
                if current.path != expected_path || current.revision != expected_revision.0 {
                    return Ok(None);
                }
                engine.refresh()?;
                plain_rs::media::audio::selection::advance_with_shuffle(
                    db,
                    &prefs,
                    action,
                    &playback::Playback {
                        path: expected_path,
                        revision: expected_revision.0,
                        position_ms: 0,
                    },
                    shuffle,
                    |path| engine.metadata(path),
                )
            })
            .await?;
        if selection.is_some() {
            let _ = ctx
                .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()?
                .send(plain_rs::events::WsEvent::broadcast(
                    "CONTENT_CHANGED",
                    "{}".into(),
                ));
        }
        Ok(selection.map(|selection| AudioSelection {
            track: selection.track.map(Into::into),
            path: selection.playback.path,
            position_ms: Long(selection.playback.position_ms),
            revision: Long(selection.playback.revision),
        }))
    }

    async fn audio_command(
        &self,
        ctx: &Context<'_>,
        action: commands::Action,
        position_ms: Long,
        speed: f32,
    ) -> Result<AudioPlaybackState> {
        let prefs = ctx.data::<Arc<Prefs>>()?.clone();
        Ok(ctx
            .data::<Arc<Audio>>()?
            .run(move |db, engine| {
                commands::command(db, &prefs, engine, action, position_ms.0, speed)
            })
            .await?
            .into())
    }
    async fn audio_play_track(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: ID,
    ) -> Result<AudioPlaybackState> {
        let prefs = ctx.data::<Arc<Prefs>>()?.clone();
        Ok(ctx
            .data::<Arc<Audio>>()?
            .run(move |db, engine| {
                engine.cache_ids(&[file_id.to_string()])?;
                let track = queue::track_by_id(db, file_id.as_str())?;
                commands::play_track(db, &prefs, engine, track)
            })
            .await?
            .into())
    }
    async fn audio_play_path(&self, ctx: &Context<'_>, path: String) -> Result<AudioPlaybackState> {
        let prefs = ctx.data::<Arc<Prefs>>()?.clone();
        Ok(ctx
            .data::<Arc<Audio>>()?
            .run(move |db, engine| commands::play_path(db, &prefs, engine, &path))
            .await?
            .into())
    }
}
