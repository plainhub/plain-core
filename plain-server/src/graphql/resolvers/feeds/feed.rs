use crate::graphql::resolvers::common::content_common;
use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::ActionResult;
use plain_graphql::{ComplexObject, Context, ID, Object, SimpleObject};

use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

use plain_rs::features::enums::DataType;
use plain_rs::features::services::feeds;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::db::notes_feeds::FeedEntryRow;
use plain_rs::infrastructure::db::notes_feeds::FeedRow;

type GqlResult<T> = plain_graphql::Result<T>;

pub use plain_rs::features::feeds::FeedError;

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
        content_common::tags(ctx, self.id.as_str(), DataType::FeedEntry.kind())
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
    pub id: ID,
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

#[derive(Default)]
pub struct FeedQuery;

#[Object]
impl FeedQuery {
    async fn fetch_feed_title(&self, ctx: &Context<'_>, url: String) -> GqlResult<String> {
        Ok(feeds::preview_title(ctx.data::<Arc<Db>>()?, &url).await?)
    }
    fn feed_sync_states(&self, ctx: &Context<'_>) -> GqlResult<Vec<feeds::FeedSyncState>> {
        Ok(sync_service(ctx)?.states())
    }
    fn feed(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<Feed>> {
        plain_rs::features::feeds::get(ctx.data::<Arc<Db>>()?, id.as_str())?
            .map(|row| feed_model(ctx, row))
            .transpose()
    }
    fn feeds(&self, ctx: &Context<'_>) -> GqlResult<Vec<Feed>> {
        plain_rs::features::feeds::list(ctx.data::<Arc<Db>>()?)?
            .into_iter()
            .map(|row| feed_model(ctx, row))
            .collect()
    }

    fn feed_entry_counts(&self, ctx: &Context<'_>) -> GqlResult<Vec<FeedEntryCount>> {
        Ok(
            plain_rs::features::feeds::entry_counts(ctx.data::<Arc<Db>>()?)?
                .into_iter()
                .map(|(id, count)| FeedEntryCount { id: ID(id), count })
                .collect(),
        )
    }

    fn feed_entry_count(&self, ctx: &Context<'_>, query: String) -> GqlResult<i32> {
        Ok(plain_rs::features::feeds::count(
            ctx.data::<Arc<Db>>()?,
            &query,
        )?)
    }

    fn feed_entry(&self, ctx: &Context<'_>, id: ID) -> GqlResult<Option<FeedEntry>> {
        plain_rs::features::feeds::entry_get(ctx.data::<Arc<Db>>()?, id.as_str())?
            .map(entry_model)
            .transpose()
    }

    fn feed_entries(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> GqlResult<Vec<FeedEntry>> {
        plain_rs::features::feeds::search(ctx.data::<Arc<Db>>()?, &query, limit, offset)?
            .into_iter()
            .map(entry_model)
            .collect()
    }
}

#[derive(Default)]
pub struct FeedMutation;

#[Object]
impl FeedMutation {
    fn mark_feed_entries_read(
        &self,
        ctx: &Context<'_>,
        query: String,
        read: bool,
    ) -> GqlResult<ActionResult> {
        Ok(ActionResult {
            affected_count: plain_rs::features::feeds::mark_read(
                ctx.data::<Arc<Db>>()?,
                &query,
                read,
            )? as i32,
        })
    }

    fn sync_feeds(&self, ctx: &Context<'_>, id: Option<ID>) -> GqlResult<bool> {
        sync_service(ctx)?.queue(id.map(|id| id.to_string()))?;
        Ok(true)
    }

    fn update_feed(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: Option<String>,
        fetch_content: Option<bool>,
        url: Option<String>,
    ) -> GqlResult<Feed> {
        feed_model(
            ctx,
            plain_rs::features::feeds::update_fields(
                ctx.data::<Arc<Db>>()?,
                id.as_str(),
                name.as_deref(),
                url.as_deref(),
                fetch_content,
            )?,
        )
    }

    async fn create_feed(
        &self,
        ctx: &Context<'_>,
        url: String,
        fetch_content: bool,
        name: Option<String>,
    ) -> GqlResult<Feed> {
        let db = ctx.data::<Arc<Db>>()?.clone();
        let feed = feeds::create_without_sync(&db, &url, fetch_content, name.as_deref()).await?;
        sync_service(ctx)?.queue(Some(feed.id.clone()))?;
        feed_model(ctx, feed)
    }

    fn import_feeds(&self, ctx: &Context<'_>, content: String) -> GqlResult<bool> {
        plain_rs::features::feeds::import_opml(ctx.data::<Arc<Db>>()?, &content)?;
        Ok(true)
    }

    fn export_feeds(&self, ctx: &Context<'_>) -> GqlResult<String> {
        Ok(plain_rs::features::feeds::export_opml(
            ctx.data::<Arc<Db>>()?,
        )?)
    }

    async fn delete_feed(&self, ctx: &Context<'_>, id: ID) -> GqlResult<bool> {
        Ok(sync_service(ctx)?.delete(id.as_str()).await?)
    }

    async fn sync_feed_entry_content(&self, ctx: &Context<'_>, id: ID) -> GqlResult<FeedEntry> {
        let assets = ctx.data_opt::<Arc<feeds::FeedAssets>>().cloned();
        entry_model(
            feeds::sync_entry_content_with_assets(
                ctx.data::<Arc<Db>>()?,
                id.as_str(),
                assets.as_deref(),
            )
            .await?,
        )
    }

    fn delete_feed_entries(&self, ctx: &Context<'_>, query: String) -> GqlResult<ActionResult> {
        let db = ctx.data::<Arc<Db>>()?;
        if query.trim().is_empty() {
            return Err("query is required".into());
        }
        let entries = plain_rs::features::feeds::search(db, &query, i32::MAX, 0)?;
        let affected_count = plain_rs::features::feeds::delete_entries(db, &query)? as i32;
        if let Some(assets) = feed_assets(ctx) {
            for entry in entries {
                assets.release(&entry.image);
            }
        }
        Ok(ActionResult { affected_count })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/feeds/feed.rs"]
mod tests;

fn sync_service(ctx: &Context<'_>) -> GqlResult<Arc<feeds::SyncService>> {
    Ok(ctx.data::<Arc<feeds::SyncService>>()?.clone())
}

fn feed_assets(ctx: &Context<'_>) -> Option<Arc<feeds::FeedAssets>> {
    ctx.data_opt::<Arc<feeds::FeedAssets>>().cloned()
}
