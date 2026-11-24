use super::{Feed, FeedEntry, FeedEntryCount, GqlResult, entry_model, feed_model, sync_service};
use plain_graphql::{Context, ID, Object};
use plain_rs::features::services::feeds;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

#[derive(Default)]
pub struct FeedQuery;

#[Object]
impl FeedQuery {
    fn export_feeds(&self, ctx: &Context<'_>) -> GqlResult<String> {
        Ok(plain_rs::features::feeds::export_opml(
            ctx.data::<Arc<Db>>()?,
        )?)
    }

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
                .map(|(feed_id, count)| FeedEntryCount {
                    feed_id: ID(feed_id),
                    count,
                })
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
