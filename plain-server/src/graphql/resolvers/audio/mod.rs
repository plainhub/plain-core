pub mod mutation;
pub mod query;

use crate::http::auth::public_gate;
use plain_graphql::{Context, SimpleObject};
use plain_rs::features::access::AccessFeature;
use plain_rs::features::media::audio::queue::{self as domain, AudioTrack};
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::audio::host::Audio as AudioService;
use plain_rs::infrastructure::db::Playlist;
use plain_rs::infrastructure::types::{Instant, Long};
use std::sync::Arc;

const FEATURE: AccessFeature = AccessFeature::Audio;

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioItem {
    #[graphql(name = "mediaId")]
    pub file_id: Option<plain_graphql::ID>,
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
pub struct AudioPlaylist {
    pub id: plain_graphql::ID,
    pub name: String,
    #[graphql(name = "itemCount")]
    pub item_count: i32,
    pub cover_candidate_paths: Vec<String>,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
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

async fn read_playlist(
    ctx: &Context<'_>,
    id: String,
) -> plain_graphql::Result<Option<AudioPlaylist>> {
    let include_covers = ctx.look_ahead().field("coverCandidatePaths").exists();
    if include_covers {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
    }
    let result = audio(ctx)?
        .run(move |db, _| {
            let Some(row) = domain::playlist_by_id(db, &id)? else {
                return Ok(None);
            };
            let count = domain::playlist_item_count(db, &id)?;
            let covers = if include_covers && count > 0 {
                plain_rs::features::media::audio::playlist_covers::playlist_cover_candidates(
                    db,
                    &[id],
                )?
                .remove(0)
            } else {
                Vec::new()
            };
            Ok(Some((row, count, covers)))
        })
        .await?;
    result
        .map(|(row, count, covers)| playlist(row, count, covers))
        .transpose()
}

fn playlist(
    row: Playlist,
    count: usize,
    cover_candidate_paths: Vec<String>,
) -> plain_graphql::Result<AudioPlaylist> {
    Ok(AudioPlaylist {
        id: row.id.into(),
        name: row.name,
        item_count: count.try_into()?,
        cover_candidate_paths,
        created_at: crate::graphql::resolvers::common::content_common::instant(&row.created_at)?,
        updated_at: crate::graphql::resolvers::common::content_common::instant(&row.updated_at)?,
    })
}

fn is_local(ctx: &Context<'_>) -> bool {
    ctx.data_opt::<crate::graphql::executor::RequestOrigin>()
        == Some(&crate::graphql::executor::RequestOrigin::Local)
}

fn audio(ctx: &Context<'_>) -> plain_graphql::Result<AudioService> {
    let service = ctx.data::<Arc<AudioService>>()?;
    Ok(if is_local(ctx) {
        service.as_ref().clone()
    } else {
        service.remote()
    })
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<Prefs> {
    ctx.data_unchecked::<Arc<Prefs>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/audio/mod.rs"]
mod tests;
