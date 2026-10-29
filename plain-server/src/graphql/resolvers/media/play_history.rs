use super::{
    item::MediaItem,
    public::{STORAGE, library_rows, prefs},
    types::MediaDataType,
};
use crate::graphql::{
    resolvers::{
        audio::public::audio_from_facts, common::content_common::instant, files::types::FileSortBy,
        videos::public::video_from_facts,
    },
    types::{Instant, Long},
};
use crate::http::auth::public_gate;
use async_graphql::{Context, ID, Object, Result, SimpleObject};
use plain_rs::{db::Db, media::play_history, utils::search_dsl};
use std::{collections::HashMap, sync::Arc};

#[derive(SimpleObject)]
pub struct MediaPlayHistory {
    #[graphql(name = "mediaId")]
    file_id: ID,
    media_type: MediaDataType,
    position_ms: Long,
    play_count: Long,
    played_at: Instant,
    media: MediaItem,
}

#[derive(Default)]
pub struct MediaPlayHistoryQuery;

#[Object]
impl MediaPlayHistoryQuery {
    async fn media_play_history(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> Result<Vec<MediaPlayHistory>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let offset = usize::try_from(offset)?;
        let limit = usize::try_from(limit)?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let (text_fields, history_fields): (Vec<_>, Vec<_>) = search_dsl::parse(&query)
            .into_iter()
            .partition(|field| field.name == "text");
        let db = ctx.data::<Arc<Db>>()?.clone();
        let history =
            tokio::task::spawn_blocking(move || play_history::matching(&db, &history_fields))
                .await??;
        let mut skipped = 0;
        let mut result = Vec::new();
        for batch in history.chunks(500) {
            let mut media = HashMap::new();
            for kind in [MediaDataType::AUDIO, MediaDataType::VIDEO] {
                let ids: Vec<_> = batch
                    .iter()
                    .filter(|row| row.media_type == kind.data_type().kind())
                    .map(|row| row.file_id.as_str())
                    .collect();
                if ids.is_empty() {
                    continue;
                }
                let query = format!("ids:{}", serde_json::to_string(&ids.join(","))?);
                let rows = library_rows(
                    ctx,
                    kind.data_type(),
                    query,
                    FileSortBy::NameAsc,
                    0,
                    ids.len() as u32,
                )
                .await?;
                for row in rows {
                    let values = ["title", "artist", "path"]
                        .map(|field| row[field].as_str().unwrap_or_default());
                    if !play_history::matches_text(&text_fields, &values) {
                        continue;
                    }
                    let item = match kind {
                        MediaDataType::AUDIO => MediaItem::Audio(audio_from_facts(&row)),
                        MediaDataType::VIDEO => MediaItem::Video(video_from_facts(&row)),
                        _ => unreachable!(),
                    };
                    media.insert(crate::graphql::types::facts::text(&row, "id"), (kind, item));
                }
            }
            for row in batch {
                let Some((media_type, media)) = media.remove(&row.file_id) else {
                    continue;
                };
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                result.push(MediaPlayHistory {
                    file_id: row.file_id.clone().into(),
                    media_type,
                    position_ms: Long(row.position_ms),
                    play_count: Long(row.play_count),
                    played_at: instant(&row.played_at)?,
                    media,
                });
                if result.len() == limit {
                    return Ok(result);
                }
            }
        }
        Ok(result)
    }
}

#[derive(Default)]
pub struct MediaPlayHistoryMutation;

#[Object]
impl MediaPlayHistoryMutation {
    async fn record_media_play(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: ID,
        r#type: MediaDataType,
        position_ms: Long,
    ) -> Result<bool> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let db = ctx.data::<Arc<Db>>()?.clone();
        tokio::task::spawn_blocking(move || {
            play_history::record(&db, r#type.data_type(), file_id.as_str(), position_ms.0)
        })
        .await??;
        if r#type == MediaDataType::AUDIO {
            crate::services::audio::reports::publish_library_updated(
                ctx.data::<Arc<crate::host::bridge::Host>>()?,
            );
        }
        Ok(true)
    }

    async fn save_media_play_progress(
        &self,
        ctx: &Context<'_>,
        #[graphql(name = "mediaId")] file_id: ID,
        r#type: MediaDataType,
        position_ms: Long,
    ) -> Result<bool> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let db = ctx.data::<Arc<Db>>()?.clone();
        tokio::task::spawn_blocking(move || {
            play_history::save(&db, r#type.data_type(), file_id.as_str(), position_ms.0)
        })
        .await??;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/media/play_history.rs"]
mod tests;
