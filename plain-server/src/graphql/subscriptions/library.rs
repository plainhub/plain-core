use super::events;
use super::models::{FeedSyncChange, MediaChange, UploadMergeResult};
use crate::graphql::resolvers::bookmarks::bookmark_types::Bookmark;
use crate::graphql::resolvers::media::types::ScanProgress;
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};

#[derive(Default)]
pub struct LibrarySubscriptions;
#[Subscription]
impl LibrarySubscriptions {
    async fn media_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<MediaChange>>> {
        events::stream(ctx, plain_rs::infrastructure::events::MEDIA_UPDATED).await
    }
    async fn media_scan_progress(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<ScanProgress>>> {
        events::stream(ctx, "MEDIA_SCAN_PROGRESS").await
    }
    async fn bookmarks_updated(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Vec<Bookmark>>>> {
        events::stream(ctx, "BOOKMARK_UPDATED").await
    }
    async fn feeds_fetched(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<FeedSyncChange>>> {
        events::stream(ctx, "FEEDS_FETCHED").await
    }
    async fn upload_merge_result(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<UploadMergeResult>>> {
        events::stream(ctx, "UPLOAD_MERGE_RESULT").await
    }
}
