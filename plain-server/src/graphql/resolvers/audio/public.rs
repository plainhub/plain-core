use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::Instant;
use crate::graphql::types::Long;
use crate::host::audio::AudioPlaybackFacts;
use crate::host::bridge::Host;
use crate::http::auth::public_gate;
use crate::services::audio::host::Audio as AudioService;
use async_graphql::{Context, Object, SimpleObject};
use plain_rs::db::Playlist;
use plain_rs::enums::DataType;
use plain_rs::media::audio::queue as domain;
use plain_rs::media::audio::queue::AudioTrack;
use serde_json::{Value, json};
use std::sync::Arc;

const STORAGE: &str = "WRITE_EXTERNAL_STORAGE";

pub use plain_rs::media::audio::play_mode::MediaPlayMode;

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioItem {
    #[graphql(name = "mediaId")]
    pub file_id: Option<async_graphql::ID>,
    pub title: String,
    pub artist: String,
    pub path: String,
    #[graphql(name = "durationMs")]
    pub duration_ms: Long,
}

impl From<AudioTrack> for AudioItem {
    fn from(track: AudioTrack) -> Self {
        item(track)
    }
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Audio {
    pub album_id: async_graphql::ID,
    pub id: async_graphql::ID,
    pub title: String,
    pub artist: String,
    pub path: String,
    #[graphql(name = "durationMs")]
    pub duration_ms: Long,
    pub size: Long,
    #[graphql(name = "bucketId")]
    pub bucket_id: async_graphql::ID,
    #[graphql(skip)]
    pub album_file_id: String,
    #[graphql(skip)]
    pub album_art_uri: String,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
    #[graphql(name = "isFavorite")]
    pub is_favorite: bool,
}

#[async_graphql::ComplexObject]
impl Audio {
    async fn album_file_id(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        if self.album_art_uri.is_empty() {
            return Ok(self.album_file_id.clone());
        }
        let token = prefs(ctx)
            .get::<String>(plain_rs::prefs::pref_keys::URL_TOKEN)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?
            .ok_or_else(|| async_graphql::Error::new("Missing URL token"))?;
        let id = plain_rs::chat::messages::content::make_file_id(&self.album_art_uri, &token);
        if id.is_empty() {
            return Err(async_graphql::Error::new("Invalid URL token"));
        }
        Ok(id)
    }

    async fn tags(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::enums::DataType::Audio,
            self.id.as_str(),
        )
        .await
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioPlaylist {
    pub id: async_graphql::ID,
    pub name: String,
    #[graphql(name = "itemCount")]
    pub item_count: i32,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioAlbumCover {
    pub album_id: async_graphql::ID,
    pub track_count: i32,
    pub candidate_paths: Vec<String>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioPlaylistCovers {
    pub id: async_graphql::ID,
    pub albums: Vec<AudioAlbumCover>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioPlayback {
    pub revision: Long,
    #[graphql(name = "mediaId")]
    pub file_id: Option<async_graphql::ID>,
    pub path: Option<String>,
    pub mode: MediaPlayMode,
    #[graphql(name = "isPlaying")]
    pub is_playing: bool,
    #[graphql(name = "positionMs")]
    pub position_ms: Long,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioArtist {
    pub name: String,
    pub item_count: i32,
    pub play_count: Long,
    pub sample_path: String,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioOverview {
    pub artists: Vec<AudioArtist>,
    pub recent_items: Vec<Audio>,
}

#[derive(Default)]
pub struct AudioQuery;

#[Object]
impl AudioQuery {
    async fn audio_overview(&self, ctx: &Context<'_>) -> async_graphql::Result<AudioOverview> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        crate::services::capabilities::require(ctx.data::<Arc<Host>>()?, "AUDIO_LIBRARY")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let mut summary = crate::services::audio::overview::read(
            ctx.data::<Arc<plain_rs::db::Db>>()?.clone(),
            crate::graphql::resolvers::media::public::catalog(ctx),
        )
        .await
        .map_err(async_graphql::Error::new)?;
        ctx.data::<Arc<crate::services::files::duration_updates::Runtime>>()?
            .enrich(DataType::Audio, &mut summary.recent_items)
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(AudioOverview {
            artists: summary
                .artists
                .into_iter()
                .map(|artist| AudioArtist {
                    name: artist.name,
                    item_count: artist.item_count,
                    play_count: Long(artist.play_count),
                    sample_path: artist.sample_path,
                })
                .collect(),
            recent_items: summary.recent_items.iter().map(audio_from_facts).collect(),
        })
    }

    async fn audio_metadata(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<AudioItem> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(item(audio(ctx)?.metadata(path).await?))
    }

    async fn audio_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[STORAGE]).await? {
            return Ok(0);
        }
        crate::graphql::resolvers::media::public::media_count(ctx, "AUDIO", query).await
    }

    async fn audios(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: crate::graphql::resolvers::files::types::FileSortBy,
    ) -> async_graphql::Result<Vec<Audio>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let items = crate::graphql::types::facts::rows(
            &crate::graphql::resolvers::media::public::media_rows(
                ctx,
                json!({
                    "dataType": "AUDIO", "query": query,
                    "offset": offset, "limit": limit, "sortBy": sort_by.as_str(),
                }),
            )
            .await?,
            |item| item.clone(),
        );
        Ok(items.iter().map(audio_from_facts).collect())
    }

    async fn audio_lyrics_by_path(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let state = ctx.data_opt::<crate::assembly::server::ServerState>();
        let lyrics = crate::services::audio::lyrics::read_for_playback(
            path,
            state.map(|state| (state.bridge.as_ref(), state.stop.clone())),
        )
        .await
        .map_err(async_graphql::Error::new)?;
        Ok(Some(lyrics).filter(|value| !value.trim().is_empty()))
    }
    async fn audio_lyrics(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: async_graphql::ID,
    ) -> async_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let path = audio(ctx)?
            .run(move |db, lib| {
                lib.cache_ids(&[file_id.to_string()])?;
                Ok(domain::track_by_id(db, file_id.as_str())?.path)
            })
            .await?;
        let lyrics = read_lyrics(ctx, path).await?;
        Ok(Some(lyrics).filter(|value| !value.trim().is_empty()))
    }
    async fn audio_queue_items(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<AudioItem>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(audio(ctx)?
            .run(move |db, _| domain::queue_page(db, offset as i64, limit as i64, &query))
            .await?
            .into_iter()
            .map(item)
            .collect())
    }

    async fn audio_queue_item_count(&self, ctx: &Context<'_>) -> async_graphql::Result<i32> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(audio(ctx)?
            .run(|db, _| domain::queue_total(db))
            .await?
            .try_into()?)
    }

    async fn audio_playlists(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Vec<AudioPlaylist>> {
        Ok(audio(ctx)?
            .run(|db, _| domain::playlists(db))
            .await?
            .into_iter()
            .map(|(row, count)| playlist(row, count))
            .collect::<async_graphql::Result<Vec<_>>>()?)
    }

    async fn audio_playlist_items(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        query: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<AudioItem>> {
        Ok(audio(ctx)?
            .run(move |db, _| {
                domain::playlist_item_records_page(
                    db,
                    id.as_str(),
                    offset as i64,
                    limit as i64,
                    &query,
                )
            })
            .await?
            .into_iter()
            .map(playlist_item)
            .collect())
    }

    async fn audio_playlist_covers(
        &self,
        ctx: &Context<'_>,
        ids: Vec<async_graphql::ID>,
    ) -> async_graphql::Result<Vec<AudioPlaylistCovers>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let ids = ids.into_iter().map(|id| id.to_string()).collect::<Vec<_>>();
        Ok(audio(ctx)?
            .run(move |db, _| plain_rs::media::audio::playlist_covers::playlist_covers(db, &ids))
            .await?
            .into_iter()
            .map(|row| AudioPlaylistCovers {
                id: row.id.into(),
                albums: row
                    .albums
                    .into_iter()
                    .map(|album| AudioAlbumCover {
                        album_id: album.album_id.into(),
                        track_count: album.track_count,
                        candidate_paths: album.candidate_paths,
                    })
                    .collect(),
            })
            .collect())
    }

    async fn audio_playlist_item_count(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        query: String,
    ) -> async_graphql::Result<i32> {
        Ok(audio(ctx)?
            .run(move |db, _| domain::playlist_filtered_count(db, id.as_str(), &query))
            .await?
            .try_into()?)
    }

    async fn audio_playback(&self, ctx: &Context<'_>) -> async_graphql::Result<AudioPlayback> {
        let (current, file_id) = audio(ctx)?
            .run(|db, _| {
                Ok((
                    plain_rs::media::audio::playback::snapshot(db)?,
                    db.audio_current_file_id(),
                ))
            })
            .await?;
        let mode = domain::play_mode(prefs(ctx))?;
        let player = if is_local(ctx) {
            ctx.data_unchecked::<Arc<Host>>()
                .audio_playback_facts()
                .await
                .map_err(async_graphql::Error::new)?
        } else {
            AudioPlaybackFacts {
                is_playing: false,
                position_ms: current.position_ms,
            }
        };
        Ok(AudioPlayback {
            revision: Long(current.revision),
            file_id: file_id.map(Into::into),
            path: Some(current.path).filter(|path| !path.is_empty()),
            mode,
            is_playing: player.is_playing,
            position_ms: Long(player.position_ms),
        })
    }
}

#[derive(Default)]
pub struct AudioMutation;

#[Object]
impl AudioMutation {
    async fn cache_audio_cover(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(audio(ctx)?
            .cover(&path)
            .await
            .map_err(async_graphql::Error::new)?
            .map(|path| path.to_string_lossy().into_owned()))
    }

    async fn play_audio(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: async_graphql::ID,
    ) -> async_graphql::Result<AudioItem> {
        let track = audio(ctx)?
            .run(move |db, lib| {
                lib.cache_ids(&[file_id.to_string()])?;
                let track = domain::track_by_id(db, file_id.as_str())?;
                plain_rs::media::audio::playback::prepare_track(db, &track)?;
                Ok(track)
            })
            .await?;
        Ok(item(track))
    }

    async fn update_audio_play_mode(
        &self,
        ctx: &Context<'_>,
        mode: MediaPlayMode,
    ) -> async_graphql::Result<bool> {
        crate::graphql::resolvers::settings::state::update(
            ctx,
            plain_rs::prefs::settings::SettingsPatch {
                audio_play_mode: Some(mode),
                ..Default::default()
            },
        )
        .await?;
        Ok(true)
    }
    async fn clear_audio_queue(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        let prefs = prefs(ctx).clone();
        let local = is_local(ctx);
        audio(ctx)?
            .run(move |db, engine| {
                domain::clear_queue(db)?;
                if local {
                    plain_rs::media::audio::commands::clear_engine(db, &prefs, engine)?;
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
        #[graphql(name = "mediaId")] file_id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        let prefs = prefs(ctx).clone();
        let local = is_local(ctx);
        audio(ctx)?
            .run(move |db, engine| {
                let current = db.audio_current_file_id().as_deref() == Some(file_id.as_str());
                domain::remove_queued(db, file_id.as_str())?;
                if local && current && domain::get_audio_current(db)?.is_empty() {
                    plain_rs::media::audio::commands::clear_engine(db, &prefs, engine)?;
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
    ) -> async_graphql::Result<bool> {
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
        let tracks = search_tracks(ctx, &query).await?;
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
        #[graphql(name = "mediaIds")] file_ids: Vec<async_graphql::ID>,
    ) -> async_graphql::Result<bool> {
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
    ) -> async_graphql::Result<AudioPlaylist> {
        let row = audio(ctx)?
            .run(move |db, _| domain::create_playlist(db, &name))
            .await?;
        playlist(row, 0)
    }

    async fn update_audio_playlist(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        name: String,
    ) -> async_graphql::Result<AudioPlaylist> {
        let key = id.to_string();
        audio(ctx)?
            .run({
                let key = key.clone();
                move |db, _| domain::rename_playlist(db, &key, &name)
            })
            .await?;
        let row = audio(ctx)?
            .run({
                let key = key.clone();
                move |db, _| domain::playlist_by_id(db, &key)
            })
            .await?
            .ok_or_else(|| {
                async_graphql::Error::new(format!("Playlist {key} not found after update"))
            })?;
        let count = audio(ctx)?
            .run(move |db, _| domain::playlist_item_count(db, &key))
            .await?;
        playlist(row, count)
    }

    async fn delete_audio_playlist(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        audio(ctx)?
            .run(move |db, _| domain::delete_playlist(db, id.as_str()))
            .await?;
        Ok(true)
    }

    async fn add_audio_playlist_items(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        query: String,
    ) -> async_graphql::Result<bool> {
        if let Some(ids) = domain::selection_ids(&query)? {
            audio(ctx)?
                .run(move |db, library| {
                    let tracks = library.tracks_by_ids(&ids)?;
                    domain::add_playlist_items(db, id.as_str(), &tracks)
                })
                .await?;
            return Ok(true);
        }
        let tracks = search_tracks(ctx, &query).await?;
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

    async fn remove_audio_playlist_item(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        query: String,
    ) -> async_graphql::Result<crate::graphql::types::ActionResult> {
        let count = audio(ctx)?
            .run(move |db, _| domain::remove_playlist_query(db, id.as_str(), &query))
            .await?;
        Ok(crate::graphql::types::ActionResult {
            affected_count: count.try_into()?,
        })
    }
}

fn notify_playback(ctx: &Context<'_>) -> async_graphql::Result<()> {
    let _ = ctx
        .data::<tokio::sync::broadcast::Sender<plain_rs::events::WsEvent>>()?
        .send(plain_rs::events::WsEvent::broadcast(
            "CONTENT_CHANGED",
            "{}".into(),
        ));
    Ok(())
}

pub(crate) async fn search_tracks(
    ctx: &Context<'_>,
    query: &str,
) -> async_graphql::Result<Vec<AudioTrack>> {
    if query.trim().is_empty() {
        return Err(async_graphql::Error::new("bulk_query_required"));
    }
    if let Some(id) = plain_rs::utils::search_dsl::field_value(query, "playlist_id") {
        let query = query.to_owned();
        return audio(ctx)?
            .run(move |db, _| domain::playlist_items_page(db, &id, 0, i64::MAX, &query))
            .await;
    }
    let sort_by = library_sort(ctx)?;
    let mut tracks = Vec::new();
    let mut offset = 0;
    loop {
        let page = crate::graphql::resolvers::media::public::library_rows(
            ctx,
            DataType::Audio,
            query.to_owned(),
            sort_by,
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
            album_id: String::new(),
            duration_ms: crate::graphql::types::facts::integer(&row, "durationMs"),
        }));
        if count < 500 {
            break;
        }
        offset += count as u32;
    }
    Ok(tracks)
}

async fn read_lyrics(ctx: &Context<'_>, path: String) -> async_graphql::Result<String> {
    let state = ctx.data_opt::<crate::assembly::server::ServerState>();
    crate::services::audio::lyrics::read(
        path,
        state.map(|state| (state.bridge.as_ref(), state.stop.clone())),
    )
    .await
    .map_err(async_graphql::Error::new)
}

fn item(track: AudioTrack) -> AudioItem {
    AudioItem {
        file_id: track.file_id.map(Into::into),
        title: track.title,
        artist: track.artist,
        path: track.path,
        duration_ms: Long(track.duration_ms),
    }
}

fn playlist_item(row: plain_rs::db::PlaylistItem) -> AudioItem {
    AudioItem {
        file_id: Some(row.file_id.into()),
        title: row.title,
        artist: row.artist,
        path: row.audio_path,
        duration_ms: Long(row.duration_ms),
    }
}

fn playlist(row: Playlist, count: usize) -> async_graphql::Result<AudioPlaylist> {
    Ok(AudioPlaylist {
        id: row.id.into(),
        name: row.name,
        item_count: count.try_into()?,
        created_at: crate::graphql::resolvers::common::content_common::instant(&row.created_at)?,
        updated_at: crate::graphql::resolvers::common::content_common::instant(&row.updated_at)?,
    })
}

pub(crate) fn is_local(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        == Some(&crate::graphql::executor::RequestOrigin::Local)
}

pub(crate) fn audio(ctx: &Context<'_>) -> async_graphql::Result<AudioService> {
    let service = ctx.data::<Arc<AudioService>>()?;
    Ok(if is_local(ctx) {
        service.as_ref().clone()
    } else {
        service.remote()
    })
}

fn library_sort(
    ctx: &Context<'_>,
) -> async_graphql::Result<crate::graphql::resolvers::files::types::FileSortBy> {
    Ok(plain_rs::prefs::settings::snapshot(prefs(ctx))?.audio_sort_by)
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<plain_rs::prefs::Prefs> {
    ctx.data_unchecked::<Arc<plain_rs::prefs::Prefs>>()
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/audio/public.rs"]
mod tests;

pub(crate) fn audio_from_facts(item: &Value) -> Audio {
    let id = crate::graphql::types::facts::id(item, "id");
    Audio {
        id: id.clone(),
        title: crate::graphql::types::facts::text(item, "title"),
        artist: plain_rs::media::audio::metadata::normalize_artist(
            &crate::graphql::types::facts::text(item, "artist"),
        ),
        path: crate::graphql::types::facts::text(item, "path"),
        duration_ms: Long(crate::graphql::types::facts::integer(item, "durationMs")),
        size: Long(crate::graphql::types::facts::integer(item, "size")),
        bucket_id: crate::graphql::types::facts::id(item, "bucketId"),
        album_id: crate::graphql::types::facts::text(item, "albumId").into(),
        album_file_id: crate::graphql::types::facts::text(item, "albumFileId"),
        album_art_uri: crate::graphql::types::facts::text(item, "albumArtUri"),
        created_at: crate::graphql::types::facts::instant(item, "createdAt"),
        updated_at: crate::graphql::types::facts::instant(item, "updatedAt"),
        is_favorite: crate::graphql::types::facts::flag(item, "isFavorite"),
    }
}
