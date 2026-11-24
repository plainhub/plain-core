use super::{AudioItem, AudioPlaylist, FEATURE, audio, item, playlist, prefs, read_playlist};
use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object, SimpleObject};
use plain_libs::serde_json::{Value, json};
use plain_rs::features::enums::DataType;
use plain_rs::features::media::audio::queue as domain;
use plain_rs::features::prefs::settings::AudioSortBy;
use plain_rs::infrastructure::types::{Instant, Long};
use std::sync::Arc;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Audio {
    pub album_id: plain_graphql::ID,
    #[graphql(getter = true)]
    pub id: plain_graphql::ID,
    #[graphql(getter = true)]
    pub title: String,
    pub artist: String,
    #[graphql(getter = true)]
    pub path: String,
    #[graphql(name = "durationMs")]
    pub duration_ms: Long,
    #[graphql(getter = true)]
    pub size: Long,
    #[graphql(name = "bucketId")]
    #[graphql(getter = true)]
    pub bucket_id: plain_graphql::ID,
    #[graphql(skip)]
    pub album_art_uri: String,
    #[graphql(name = "createdAt")]
    #[graphql(getter = true)]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    #[graphql(getter = true)]
    pub updated_at: Instant,
    #[graphql(name = "isFavorite")]
    pub is_favorite: bool,
}

#[plain_graphql::ComplexObject]
impl Audio {
    fn album_file_id(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        if self.album_art_uri.is_empty() {
            return Ok(String::new());
        }
        let resource =
            plain_rs::features::files::resource_link::Resource::reference(&self.album_art_uri);
        let (scope, token) = crate::graphql::executor::resource_credentials(ctx)?;
        plain_rs::features::files::resource_link::seal(
            &scope,
            &plain_libs::utils::base64::base64_decode(&token),
            &resource,
        )
        .map_err(|error| plain_graphql::Error::from(error))
    }

    async fn tags(&self, ctx: &plain_graphql::Context<'_>) -> plain_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::features::enums::DataType::Audio,
            self.id.as_str(),
        )
        .await
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioPlayback {
    #[graphql(name = "mediaId")]
    pub file_id: Option<plain_graphql::ID>,
    pub path: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioArtist {
    pub name: String,
    pub item_count: i32,
    pub play_count: Long,
    pub sample_path: String,
}

#[derive(Default)]
pub struct AudioQuery;

#[Object]
impl AudioQuery {
    async fn audio_artists(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<AudioArtist>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        let artists = plain_rs::features::services::audio::overview::read(
            ctx.data::<Arc<plain_rs::infrastructure::db::Db>>()?.clone(),
            crate::graphql::resolvers::media::query::catalog(ctx, DataType::Audio).await?,
        )
        .await
        .map_err(plain_graphql::Error::new)?;
        Ok(artists
            .into_iter()
            .map(|artist| AudioArtist {
                name: artist.name,
                item_count: artist.item_count,
                play_count: Long(artist.play_count),
                sample_path: artist.sample_path,
            })
            .collect())
    }

    async fn audio_by_path(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> plain_graphql::Result<AudioItem> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        Ok(item(audio(ctx)?.metadata(path).await?))
    }

    async fn audio_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[FEATURE]).await? {
            return Ok(0);
        }
        crate::graphql::resolvers::media::query::media_count(ctx, "AUDIO", query).await
    }

    async fn audios(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: AudioSortBy,
    ) -> plain_graphql::Result<Vec<Audio>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        let items = crate::graphql::types::facts::rows(
            &crate::graphql::resolvers::media::query::media_rows(
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
    ) -> plain_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        let state = ctx.data_opt::<crate::server::ServerState>();
        let lyrics = plain_rs::features::services::audio::lyrics::read_for_playback(
            path,
            state.map(|state| (state.resources.as_ref(), state.stop.clone())),
        )
        .await
        .map_err(plain_graphql::Error::new)?;
        Ok(Some(lyrics).filter(|value| !value.trim().is_empty()))
    }
    async fn audio_lyrics(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: plain_graphql::ID,
    ) -> plain_graphql::Result<Option<String>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
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
    ) -> plain_graphql::Result<Vec<AudioItem>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        Ok(audio(ctx)?
            .run(move |db, _| domain::queue_page(db, offset as i64, limit as i64, &query))
            .await?
            .into_iter()
            .map(item)
            .collect())
    }

    async fn audio_queue_item_count(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<i32> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        Ok(audio(ctx)?
            .run(move |db, _| domain::queue_filtered_count(db, &query))
            .await?
            .try_into()?)
    }

    async fn audio_playlists(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<Vec<AudioPlaylist>> {
        let include_covers = ctx.look_ahead().field("coverCandidatePaths").exists();
        if include_covers {
            public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        }
        let (rows, mut covers) = audio(ctx)?
            .run(move |db, _| {
                let rows = domain::playlists(db)?;
                let ids = rows
                    .iter()
                    .filter(|(_, count)| include_covers && *count > 0)
                    .map(|(row, _)| row.id.clone())
                    .collect::<Vec<_>>();
                let covers =
                    plain_rs::features::media::audio::playlist_covers::playlist_cover_candidates(
                        db, &ids,
                    )?;
                Ok((
                    rows,
                    ids.into_iter()
                        .zip(covers)
                        .collect::<std::collections::HashMap<_, _>>(),
                ))
            })
            .await?;
        rows.into_iter()
            .map(|(row, count)| {
                let candidates = covers.remove(&row.id).unwrap_or_default();
                playlist(row, count, candidates)
            })
            .collect()
    }

    async fn audio_playlist(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
    ) -> plain_graphql::Result<Option<AudioPlaylist>> {
        read_playlist(ctx, id.to_string()).await
    }

    async fn audio_playlist_items(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<AudioItem>> {
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

    async fn audio_playlist_item_count(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        query: String,
    ) -> plain_graphql::Result<i32> {
        Ok(audio(ctx)?
            .run(move |db, _| domain::playlist_filtered_count(db, id.as_str(), &query))
            .await?
            .try_into()?)
    }

    async fn audio_playback(&self, ctx: &Context<'_>) -> plain_graphql::Result<AudioPlayback> {
        let (current, file_id) = audio(ctx)?
            .run(|db, _| {
                Ok((
                    plain_rs::features::media::audio::playback::snapshot(db)?,
                    db.audio_current_file_id(),
                ))
            })
            .await?;
        Ok(AudioPlayback {
            file_id: file_id.map(Into::into),
            path: Some(current.path).filter(|path| !path.is_empty()),
        })
    }
}

async fn read_lyrics(ctx: &Context<'_>, path: String) -> plain_graphql::Result<String> {
    let state = ctx.data_opt::<crate::server::ServerState>();
    plain_rs::features::services::audio::lyrics::read(
        path,
        state.map(|state| (state.resources.as_ref(), state.stop.clone())),
    )
    .await
    .map_err(plain_graphql::Error::new)
}

fn playlist_item(row: plain_rs::infrastructure::db::PlaylistItem) -> AudioItem {
    AudioItem {
        file_id: Some(row.file_id.into()),
        title: row.title,
        artist: row.artist,
        path: row.audio_path,
        duration_ms: Long(row.duration_ms),
    }
}

pub(crate) fn audio_from_facts(item: &Value) -> Audio {
    let id = crate::graphql::types::facts::id(item, "id");
    Audio {
        id: id.clone(),
        title: crate::graphql::types::facts::text(item, "title"),
        artist: plain_rs::features::media::audio::metadata::normalize_artist(
            &crate::graphql::types::facts::text(item, "artist"),
        ),
        path: crate::graphql::types::facts::text(item, "path"),
        duration_ms: Long(crate::graphql::types::facts::integer(item, "durationMs")),
        size: Long(crate::graphql::types::facts::integer(item, "size")),
        bucket_id: crate::graphql::types::facts::id(item, "bucketId"),
        album_id: crate::graphql::types::facts::text(item, "albumId").into(),
        album_art_uri: crate::graphql::types::facts::text(item, "albumArtUri"),
        created_at: crate::graphql::types::facts::instant(item, "createdAt"),
        updated_at: crate::graphql::types::facts::instant(item, "updatedAt"),
        is_favorite: crate::graphql::types::facts::flag(item, "isFavorite"),
    }
}
