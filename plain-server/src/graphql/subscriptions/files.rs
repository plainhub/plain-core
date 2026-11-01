use super::events;
use crate::graphql::executor::RequestOrigin;
use crate::graphql::resolvers::files::types::File;
use crate::server::ServerState;
use async_graphql::{Context, Result, SimpleObject, Subscription};
use futures_util::{
    StreamExt,
    stream::{self, BoxStream},
};
use plain_rs::features::prefs::settings::FileSortBy;
use plain_rs::libs::filesystem::browse::Request;
use plain_rs::libs::protocols::graphql::events::EventSpec;
use plain_rs::libs::types::{Instant, Long};

pub(super) const SPEC: EventSpec = EventSpec {
    field: "fileSearchResults",
    event: "",
    aliases: &[],
    invalidation: false,
    selection: "",
    permissions: &["WRITE_EXTERNAL_STORAGE"],
    capability: None,
};

#[derive(SimpleObject)]
pub struct FileSearchResults {
    items: Vec<File>,
    count: Long,
    complete: bool,
}

#[derive(Default)]
pub struct FileSubscriptions;

#[Subscription]
impl FileSubscriptions {
    async fn file_search_results(
        &self,
        ctx: &Context<'_>,
        root: String,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: FileSortBy,
    ) -> Result<BoxStream<'static, Result<FileSearchResults>>> {
        let state = ctx.data::<ServerState>()?.clone();
        let origin = *ctx.data::<RequestOrigin>()?;
        events::authorize(&state, origin, &SPEC).await?;
        let receiver = plain_rs::features::services::files::browse::search(
            ctx.data::<std::sync::Arc<plain_rs::features::services::files::platform::FilePlatform>>()?,
            Request {
                root, query, sort_by: sort_by.into(),
                offset: offset.max(0) as usize,
                limit: Some(limit.max(0) as usize), count_only: false,
            },
        ).await.map_err(async_graphql::Error::new)?;
        if let Some(ready) = ctx.data_opt::<super::session::SubscriptionReady>() {
            ready.0.send_replace(true);
        }
        Ok(stream::unfold(
            (receiver, state, false),
            move |(mut receiver, state, ended)| async move {
                if ended {
                    return None;
                }
                let received = receiver.recv().await?;
                let result = match events::authorize(&state, origin, &SPEC).await {
                    Err(error) => Err(error),
                    Ok(()) => received
                        .map(|(page, complete)| FileSearchResults {
                            items: page
                                .items
                                .into_iter()
                                .map(|item| File {
                                    permission: item.permission,
                                    file_id: (!item.file_id.is_empty())
                                        .then(|| item.file_id.into()),
                                    name: item.name,
                                    path: item.path,
                                    created_at: item
                                        .created_at
                                        .and_then(chrono::DateTime::from_timestamp_millis)
                                        .map(Instant),
                                    updated_at: Instant(
                                        chrono::DateTime::from_timestamp_millis(item.updated_at)
                                            .unwrap_or_default(),
                                    ),
                                    size: Long(item.size),
                                    is_dir: item.is_dir,
                                    child_count: item.child_count,
                                })
                                .collect(),
                            count: Long(page.count as i64),
                            complete,
                        })
                        .map_err(async_graphql::Error::new),
                };
                let ended = result.as_ref().map_or(true, |page| page.complete);
                Some((result, (receiver, state, ended)))
            },
        )
        .boxed())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/graphql/subscriptions/files.rs"]
mod tests;
