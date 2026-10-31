use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::facts::flag;
use crate::graphql::types::facts::optional_instant;
use crate::graphql::types::facts::rows;
use crate::http::auth::public_gate;
use async_graphql::{Context, Object, SimpleObject};
use plain_rs::types::Instant;
use plain_rs::types::Long;
use serde_json::Value;
use std::sync::Arc;

use crate::graphql::resolvers::media::public::STORAGE;
use crate::graphql::resolvers::media::public::prefs;
use crate::graphql::resolvers::media::public::row;
use crate::graphql::resolvers::media::public::text_field;

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Image {
    pub id: async_graphql::ID,
    pub title: String,
    pub path: String,
    pub size: Long,
    pub width: i32,
    pub height: i32,
    pub rotation: i32,
    #[graphql(name = "bucketId")]
    pub bucket_id: async_graphql::ID,
    #[graphql(name = "createdAt")]
    pub created_at: Instant,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
    #[graphql(name = "takenAt")]
    pub taken_at: Option<Instant>,
    #[graphql(name = "isFavorite")]
    pub is_favorite: bool,
}

#[async_graphql::ComplexObject]
impl Image {
    async fn tags(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::enums::DataType::Image,
            self.id.as_str(),
        )
        .await
    }
}

#[derive(Default)]
pub struct ImagesQuery;

#[Object]
impl ImagesQuery {
    async fn image_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[STORAGE]).await? {
            return Ok(0);
        }
        plain_rs::services::images::search::count(
            &crate::graphql::resolvers::media::public::catalog(ctx),
            ctx.data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>(),
            &text_field(&query),
            &query,
        )
        .await
        .map_err(async_graphql::Error::new)
    }

    async fn images(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: plain_rs::prefs::settings::FileSortBy,
    ) -> async_graphql::Result<Vec<Image>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let items = Value::Array(
            plain_rs::services::images::search::rows(
                &crate::graphql::resolvers::media::public::catalog(ctx),
                ctx.data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>(),
                &text_field(&query),
                &query,
                offset,
                limit,
                sort_by.as_str(),
            )
            .await
            .map_err(async_graphql::Error::new)?,
        );
        Ok(rows(&items, image))
    }
}

fn image(item: &Value) -> Image {
    let row = row(item);
    Image {
        id: row.id.into(),
        title: row.title,
        path: row.path,
        size: row.size,
        width: crate::graphql::types::facts::integer(item, "width") as i32,
        height: crate::graphql::types::facts::integer(item, "height") as i32,
        rotation: crate::graphql::types::facts::integer(item, "rotation") as i32,
        bucket_id: row.bucket_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        taken_at: optional_instant(item, "takenAt"),
        is_favorite: flag(item, "isFavorite"),
    }
}
