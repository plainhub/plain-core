use plain_rs::features::access::AccessFeature;
use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::facts::flag;
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::optional_instant;
use crate::graphql::types::facts::rows;
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object, SimpleObject};

use plain_rs::infrastructure::types::Instant;
use plain_rs::infrastructure::types::Long;
use plain_libs::serde_json::{Value, json};

const FEATURE: AccessFeature = AccessFeature::Videos;
use crate::graphql::resolvers::media::public::media_count;
use crate::graphql::resolvers::media::public::media_rows;
use crate::graphql::resolvers::media::public::prefs;
use crate::graphql::resolvers::media::public::row;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Video {
    pub width: i32,
    pub height: i32,
    pub rotation: i32,
    #[graphql(getter = true)]
    pub id: plain_graphql::ID,
    #[graphql(getter = true)]
    pub title: String,
    #[graphql(getter = true)]
    pub path: String,
    #[graphql(name = "durationMs")]
    pub duration_ms: Long,
    #[graphql(getter = true)]
    pub size: Long,
    #[graphql(name = "bucketId")]
    #[graphql(getter = true)]
    pub bucket_id: plain_graphql::ID,
    #[graphql(name = "createdAt")]
    #[graphql(getter = true)]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    #[graphql(getter = true)]
    pub updated_at: Instant,
    #[graphql(name = "takenAt")]
    pub taken_at: Option<Instant>,
    #[graphql(name = "isFavorite")]
    pub is_favorite: bool,
}

#[plain_graphql::ComplexObject]
impl Video {
    async fn tags(&self, ctx: &plain_graphql::Context<'_>) -> plain_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::features::enums::DataType::Video,
            self.id.as_str(),
        )
        .await
    }
}

#[derive(Default)]
pub struct VideosQuery;

#[Object]
impl VideosQuery {
    async fn video_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[FEATURE]).await? {
            return Ok(0);
        }
        media_count(ctx, "VIDEO", query).await
    }

    async fn videos(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: plain_rs::features::prefs::settings::FileSortBy,
    ) -> plain_graphql::Result<Vec<Video>> {
        public_gate::require(ctx, prefs(ctx), &[FEATURE])?;
        let items = media_rows(
            ctx,
            json!({
                "dataType": "VIDEO", "query": query,
                "offset": offset, "limit": limit, "sortBy": sort_by.as_str(),
            }),
        )
        .await?;
        Ok(rows(&items, video_from_facts))
    }
}

pub(crate) fn video_from_facts(item: &Value) -> Video {
    let row = row(item);
    Video {
        width: integer(item, "width") as i32,
        height: integer(item, "height") as i32,
        rotation: integer(item, "rotation") as i32,
        id: row.id,
        title: row.title,
        path: row.path,
        duration_ms: Long(integer(item, "durationMs")),
        size: row.size,
        bucket_id: row.bucket_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        taken_at: optional_instant(item, "takenAt"),
        is_favorite: flag(item, "isFavorite"),
    }
}
