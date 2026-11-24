pub mod mutation;
pub mod query;

use crate::graphql::resolvers::common::content_common;
use crate::graphql::resolvers::tags::domain_types::Tag;
use plain_graphql::{ComplexObject, Context, ID, SimpleObject};
use plain_rs::features::enums::DataType;
pub use plain_rs::features::feeds::FeedError;
use plain_rs::features::services::feeds;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::db::notes_feeds::{FeedEntryRow, FeedRow};
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

type GqlResult<T> = plain_graphql::Result<T>;

#[derive(SimpleObject, Clone)]
pub struct Feed {
    pub id: ID,
    pub name: String,
    pub url: String,
    pub fetch_content: bool,
    pub logo: String,
    pub last_sync_at: Option<Instant>,
    pub last_error: FeedError,
    pub created_at: Instant,
    pub updated_at: Instant,
}

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct FeedEntry {
    pub id: ID,
    pub title: String,
    pub url: String,
    pub image: String,
    pub description: String,
    pub author: String,
    pub content: String,
    pub feed_id: ID,
    pub raw_id: String,
    pub read: bool,
    pub published_at: Instant,
    pub created_at: Instant,
    pub updated_at: Instant,
}

#[ComplexObject]
impl FeedEntry {
    async fn tags(&self, ctx: &Context<'_>) -> GqlResult<Vec<Tag>> {
        #[cfg(feature = "core")]
        if ctx
            .data_opt::<Arc<crate::graphql::resolvers::tags::loader::TagsLoader>>()
            .is_some()
        {
            return crate::graphql::resolvers::tags::loader::load(
                ctx,
                DataType::FeedEntry,
                self.id.as_str(),
            )
            .await;
        }
        content_common::tags(ctx, self.id.as_str(), DataType::FeedEntry)
    }

    fn feed(&self, ctx: &Context<'_>) -> GqlResult<Option<Feed>> {
        let db = ctx.data::<Arc<Db>>()?;
        plain_rs::features::feeds::get(db, self.feed_id.as_str())?
            .map(|row| feed_model(ctx, row))
            .transpose()
    }
}

#[derive(SimpleObject, Clone)]
pub struct FeedEntryCount {
    pub feed_id: ID,
    pub count: i32,
}

fn feed_model(_ctx: &Context<'_>, row: FeedRow) -> GqlResult<Feed> {
    Ok(Feed {
        id: ID(row.id),
        name: row.name,
        url: row.url,
        fetch_content: row.fetch_full_content,
        logo: row.logo,
        last_sync_at: row
            .last_sync_at
            .as_deref()
            .map(content_common::instant)
            .transpose()?,
        last_error: row.last_error,
        created_at: content_common::instant(&row.created_at)?,
        updated_at: content_common::instant(&row.updated_at)?,
    })
}

fn entry_model(row: FeedEntryRow) -> GqlResult<FeedEntry> {
    Ok(FeedEntry {
        id: ID(row.id),
        feed_id: ID(row.feed_id),
        title: row.title,
        url: row.url,
        image: row.image,
        description: row.description,
        author: row.author,
        content: row.content,
        raw_id: row.source_entry_id,
        read: row.is_read,
        published_at: content_common::instant(&row.published_at)?,
        created_at: content_common::instant(&row.created_at)?,
        updated_at: content_common::instant(&row.updated_at)?,
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/feeds/mod.rs"]
mod tests;

fn sync_service(ctx: &Context<'_>) -> GqlResult<Arc<feeds::SyncService>> {
    Ok(ctx.data::<Arc<feeds::SyncService>>()?.clone())
}
