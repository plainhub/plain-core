use super::super::item::MediaItem;
use super::super::query::{library_rows, prefs};
use super::{MediaPlayHistory, PlaybackType};
use crate::graphql::resolvers::audio::query::audio_from_facts;
use crate::graphql::resolvers::common::content_common::instant;
use crate::graphql::resolvers::videos::query::video_from_facts;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object, Result};
use plain_libs::utils::search_dsl;
use plain_rs::features::access_error::AccessErrorCode;
use plain_rs::features::media::play_history;
use plain_rs::features::prefs::settings::MediaSortBy;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::types::Long;
use std::collections::HashMap;
use std::sync::Arc;

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
        let allowed: Vec<_> = [PlaybackType::Audio, PlaybackType::Video]
            .into_iter()
            .filter(|kind| {
                public_gate::is_local(ctx)
                    || plain_rs::features::services::device::access::is_enabled(
                        prefs(ctx),
                        &[super::super::query::access_feature(kind.data_type())],
                    )
            })
            .collect();
        if allowed.is_empty() {
            return Err(AccessErrorCode::NoPermission.into());
        }

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
            for kind in allowed.iter().copied() {
                let ids: Vec<_> = batch
                    .iter()
                    .filter(|row| row.media_type == kind.data_type())
                    .map(|row| row.file_id.as_str())
                    .collect();
                if ids.is_empty() {
                    continue;
                }
                let query = format!("ids:{}", plain_libs::serde_json::to_string(&ids.join(","))?);
                let rows = library_rows(
                    ctx,
                    kind.data_type(),
                    query,
                    MediaSortBy::UpdatedAtAsc,
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
                        PlaybackType::Audio => MediaItem::Audio(audio_from_facts(&row)),
                        PlaybackType::Video => MediaItem::Video(video_from_facts(&row)),
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
