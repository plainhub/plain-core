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
    offset: Long,
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
            (receiver, state, false, std::collections::VecDeque::new()),
            move |(mut receiver, state, ended, mut pending)| async move {
                if ended {
                    return None;
                }
                if pending.is_empty() {
                    let received = receiver.recv().await?;
                    match received {
                        Ok((page, complete)) => pending = batches(page, complete),
                        Err(error) => pending.push_back(Err(async_graphql::Error::new(error))),
                    }
                }
                let result = match events::authorize(&state, origin, &SPEC).await {
                    Err(error) => Err(error),
                    Ok(()) => pending.pop_front()?,
                };
                let ended = result.as_ref().map_or(true, |page| page.complete);
                Some((result, (receiver, state, ended, pending)))
            },
        )
        .boxed())
    }
}

fn batches(
    page: plain_rs::libs::filesystem::browse::Page,
    complete: bool,
) -> std::collections::VecDeque<Result<FileSearchResults>> {
    let mut batches = std::collections::VecDeque::new();
    let mut batch = FileSearchResults {
        items: Vec::new(),
        offset: Long(0),
        count: Long(page.count as i64),
        complete: false,
    };
    let mut bytes = 0;
    for item in page.items {
        let size = serde_json::to_vec(&item)
            .expect("file records serialize")
            .len()
            + 128;
        if !batch.items.is_empty() && (batch.items.len() == 128 || bytes + size > 256 * 1024) {
            let offset = batch.offset.0 + batch.items.len() as i64;
            batches.push_back(Ok(batch));
            batch = FileSearchResults {
                items: Vec::new(),
                offset: Long(offset),
                count: Long(page.count as i64),
                complete: false,
            };
            bytes = 0;
        }
        bytes += size;
        batch.items.push(File {
            permission: item.permission,
            file_id: (!item.file_id.is_empty()).then(|| item.file_id.into()),
            name: item.name,
            path: item.path,
            created_at: item
                .created_at
                .and_then(chrono::DateTime::from_timestamp_millis)
                .map(Instant),
            updated_at: Instant(
                chrono::DateTime::from_timestamp_millis(item.updated_at).unwrap_or_default(),
            ),
            size: Long(item.size),
            is_dir: item.is_dir,
            child_count: item.child_count,
        });
    }
    batch.complete = complete;
    batches.push_back(Ok(batch));
    batches
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/graphql/subscriptions/files.rs"]
mod tests;
