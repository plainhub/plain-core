use super::{
    AudioItem, AudioPlaylist, FEATURE, audio, is_local, item, playlist, prefs, read_playlist,
};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_rs::features::enums::DataType;
use plain_rs::features::media::audio::error::AudioQueueErrorCode;
use plain_rs::features::media::audio::play_mode::MediaPlayMode;
use plain_rs::features::media::audio::queue::{self as domain, AudioTrack};
use plain_rs::features::prefs::settings::AudioSortBy;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct AudioMutation;

#[Object]
impl AudioMutation {
    async fn select_audio(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: plain_graphql::ID,
    ) -> plain_graphql::Result<AudioItem> {
        let (track, selection) = audio(ctx)?
            .run(move |db, lib| {
                lib.cache_ids(&[file_id.to_string()])?;
                let track = domain::track_by_id(db, file_id.as_str())?;
                let selection =
                    plain_rs::features::media::audio::playback::prepare_track(db, &track)?;
                Ok((track, selection))
            })
            .await?;
        stage_audio_selection(ctx, selection).await?;
        Ok(item(track))
    }

    async fn select_audio_by_path(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> plain_graphql::Result<AudioItem> {
        use plain_rs::features::media::audio::commands::Engine;
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        let (track, selection) = audio(ctx)?
            .run(move |db, library| {
                let track = library.metadata(&path)?;
                let selection =
                    plain_rs::features::media::audio::playback::prepare_track(db, &track)?;
                Ok((track, selection))
            })
            .await?;
        stage_audio_selection(ctx, selection).await?;
        Ok(item(track))
    }

    async fn update_audio_play_mode(
        &self,
        ctx: &Context<'_>,
        mode: MediaPlayMode,
    ) -> plain_graphql::Result<bool> {
        crate::graphql::resolvers::settings::state::update(
            ctx,
            plain_rs::features::prefs::settings::SettingsPatch {
                audio_play_mode: Some(mode),
                ..Default::default()
            },
        )
        .await?;
        Ok(true)
    }
    async fn replace_audio_queue(
        &self,
        ctx: &Context<'_>,
        query: String,
        sort_by: AudioSortBy,
        shuffle: bool,
    ) -> plain_graphql::Result<Option<AudioItem>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        let tracks = search_tracks(ctx, &query, sort_by).await?;
        let prefs = prefs(ctx).clone();
        let local = is_local(ctx);
        let start = audio(ctx)?
            .run(move |db, library| {
                library.cache_ids(
                    &tracks
                        .iter()
                        .filter_map(|track| track.file_id.clone())
                        .collect::<Vec<_>>(),
                )?;
                let start = domain::replace_queue(db, &tracks, shuffle)?;
                if local && start.is_none() {
                    plain_rs::features::media::audio::commands::clear_engine(db, &prefs, library)?;
                }
                Ok(start)
            })
            .await?;
        notify_playback(ctx)?;
        Ok(start.map(item))
    }

    async fn clear_audio_queue(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        let prefs = prefs(ctx).clone();
        let local = is_local(ctx);
        audio(ctx)?
            .run(move |db, engine| {
                domain::clear_queue(db)?;
                if local {
                    plain_rs::features::media::audio::commands::clear_engine(db, &prefs, engine)?;
                }
                Ok(())
            })
            .await?;
        notify_playback(ctx)?;
        Ok(true)
    }

    async fn remove_audio_from_queue(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        let prefs = prefs(ctx).clone();
        let local = is_local(ctx);
        audio(ctx)?
            .run(move |db, engine| {
                let current = db.audio_current_file_id().as_deref() == Some(file_id.as_str());
                domain::remove_queued(db, file_id.as_str())?;
                if local && current && domain::get_audio_current(db)?.is_empty() {
                    plain_rs::features::media::audio::commands::clear_engine(db, &prefs, engine)?;
                }
                Ok(())
            })
            .await?;
        notify_playback(ctx)?;
        Ok(true)
    }

    async fn add_audio_to_queue(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<bool> {
        if let Some(ids) = domain::selection_ids(&query)? {
            audio(ctx)?
                .run(move |db, library| {
                    let tracks = library.tracks_by_ids(&ids)?;
                    domain::enqueue_selecting_if_idle(db, &tracks)
                })
                .await?;
            notify_playback(ctx)?;
            return Ok(true);
        }
        let tracks = search_tracks(ctx, &query, library_sort(ctx)?).await?;
        audio(ctx)?
            .run(move |db, lib| {
                lib.cache_ids(
                    &tracks
                        .iter()
                        .filter_map(|track| track.file_id.clone())
                        .collect::<Vec<_>>(),
                )?;
                domain::enqueue_selecting_if_idle(db, &tracks)
            })
            .await?;
        notify_playback(ctx)?;
        Ok(true)
    }

    async fn reorder_audio_queue(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaIds")] file_ids: Vec<plain_graphql::ID>,
    ) -> plain_graphql::Result<bool> {
        audio(ctx)?
            .run(move |db, _| {
                domain::reorder_queued(
                    db,
                    &file_ids
                        .into_iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>(),
                )
            })
            .await?;
        Ok(true)
    }

    async fn create_audio_playlist(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> plain_graphql::Result<AudioPlaylist> {
        let row = audio(ctx)?
            .run(move |db, _| domain::create_playlist(db, &name))
            .await?;
        playlist(row, 0, Vec::new())
    }

    async fn update_audio_playlist(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        name: String,
    ) -> plain_graphql::Result<AudioPlaylist> {
        let key = id.to_string();
        audio(ctx)?
            .run({
                let key = key.clone();
                move |db, _| domain::rename_playlist(db, &key, &name)
            })
            .await?;
        read_playlist(ctx, key.clone()).await?.ok_or_else(|| {
            plain_graphql::Error::new(format!("Playlist {key} not found after update"))
        })
    }

    async fn delete_audio_playlist(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        audio(ctx)?
            .run(move |db, _| domain::delete_playlist(db, id.as_str()))
            .await?;
        Ok(true)
    }

    async fn add_audio_playlist_items(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        query: String,
    ) -> plain_graphql::Result<bool> {
        if let Some(ids) = domain::selection_ids(&query)? {
            audio(ctx)?
                .run(move |db, library| {
                    let tracks = library.tracks_by_ids(&ids)?;
                    domain::add_playlist_items(db, id.as_str(), &tracks)
                })
                .await?;
            return Ok(true);
        }
        let tracks = search_tracks(ctx, &query, library_sort(ctx)?).await?;
        audio(ctx)?
            .run(move |db, lib| {
                lib.cache_ids(
                    &tracks
                        .iter()
                        .filter_map(|track| track.file_id.clone())
                        .collect::<Vec<_>>(),
                )?;
                domain::add_playlist_items(db, id.as_str(), &tracks)
            })
            .await?;
        Ok(true)
    }

    async fn remove_audio_playlist_items(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        query: String,
    ) -> plain_graphql::Result<crate::graphql::types::ActionResult> {
        let count = audio(ctx)?
            .run(move |db, _| domain::remove_playlist_query(db, id.as_str(), &query))
            .await?;
        Ok(crate::graphql::types::ActionResult {
            affected_count: count.try_into()?,
        })
    }
}

fn notify_playback(ctx: &Context<'_>) -> plain_graphql::Result<()> {
    let _ = ctx
        .data::<tokio::sync::broadcast::Sender<plain_rs::infrastructure::events::WsEvent>>()?
        .send(plain_rs::infrastructure::events::WsEvent::broadcast(
            "CONTENT_CHANGED",
            "{}".into(),
        ));
    Ok(())
}

async fn search_tracks(
    ctx: &Context<'_>,
    query: &str,
    sort_by: AudioSortBy,
) -> plain_graphql::Result<Vec<AudioTrack>> {
    if query.trim().is_empty() {
        return Err(plain_graphql::Error::from(
            AudioQueueErrorCode::BulkQueryRequired,
        ));
    }
    if let Some(id) = plain_libs::utils::search_dsl::field_value(query, "playlist_id") {
        let query = query.to_owned();
        return audio(ctx)?
            .run(move |db, _| domain::playlist_items_page(db, &id, 0, i64::MAX, &query))
            .await
            .map_err(Into::into);
    }
    let mut tracks = Vec::new();
    let mut offset = 0;
    loop {
        let page = crate::graphql::resolvers::media::query::library_rows(
            ctx,
            DataType::Audio,
            query.to_owned(),
            sort_by.into(),
            offset,
            500,
        )
        .await?;
        let count = page.len();
        tracks.extend(page.into_iter().map(|row| AudioTrack {
            file_id: row["id"].as_str().map(str::to_owned),
            path: crate::graphql::types::facts::text(&row, "path"),
            title: crate::graphql::types::facts::text(&row, "title"),
            artist: crate::graphql::types::facts::text(&row, "artist"),
            duration_ms: crate::graphql::types::facts::integer(&row, "durationMs"),
        }));
        if count < 500 {
            break;
        }
        offset += count as u32;
    }
    Ok(tracks)
}

async fn stage_audio_selection(
    ctx: &Context<'_>,
    selection: plain_rs::features::media::audio::playback::Playback,
) -> plain_graphql::Result<()> {
    let host = ctx.data::<Arc<Host>>()?;
    if is_local(ctx) && host.connected() {
        let params = plain_libs::serde_json::to_value(selection)
            .map_err(|error| plain_graphql::Error::from(error))?;
        host.call("audioPlaybackRefresh", params)
            .await
            .map_err(plain_graphql::Error::new)?;
    }
    Ok(())
}

fn library_sort(ctx: &Context<'_>) -> plain_graphql::Result<AudioSortBy> {
    Ok(plain_rs::features::prefs::settings::snapshot(prefs(ctx))?.audio_sort_by)
}
