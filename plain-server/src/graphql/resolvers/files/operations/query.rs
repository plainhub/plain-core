use super::{MergeTask, STORAGE, UploadedChunk, merge_task, prefs};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_rs::infrastructure::types::Long;
use std::sync::Arc;

#[derive(Default)]
pub struct UploadQuery;

#[Object]
impl UploadQuery {
    async fn uploaded_chunks(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> plain_graphql::Result<Vec<UploadedChunk>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        ctx.data::<Arc<plain_rs::features::services::files::upload_chunks::UploadChunks>>()?
            .list(&file_id)
            .await
            .map_err(plain_graphql::Error::new)?
            .into_iter()
            .map(|chunk| {
                Ok(UploadedChunk {
                    index: chunk.index.try_into()?,
                    size_bytes: Long(chunk.size_bytes.try_into()?),
                })
            })
            .collect()
    }

    /// `NONE` when no merge was ever started for this id, which is what a
    /// client sees before it calls `mergeChunks`.
    fn merge_status(&self, ctx: &Context<'_>, file_id: String) -> plain_graphql::Result<MergeTask> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        Ok(merge_task(
            &ctx.data_unchecked::<std::sync::Arc<plain_rs::features::files::uploads::Runtime>>()
                .status(&file_id),
        ))
    }
}
