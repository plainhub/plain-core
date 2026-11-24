use super::{Feed, FeedEntry, GqlResult, entry_model, feed_model, sync_service};
use crate::graphql::types::ActionResult;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::services::feeds;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

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

fn feed_assets(ctx: &Context<'_>) -> Option<Arc<feeds::FeedAssets>> {
    ctx.data_opt::<Arc<feeds::FeedAssets>>().cloned()
}
