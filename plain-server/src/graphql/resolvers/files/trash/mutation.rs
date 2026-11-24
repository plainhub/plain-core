use super::tasks;
use crate::graphql::types::ActionResult;
use plain_graphql::{Context, Object, Result};

#[derive(Default)]
pub struct TrashMutation;

#[Object]
impl TrashMutation {
    async fn trash_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.trash_paths(paths).await?.try_into()?,
        })
    }
    async fn restore_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.restore_paths(paths).await?.try_into()?,
        })
    }
    async fn delete_trashed_files(
        &self,
        ctx: &Context<'_>,
        paths: Vec<String>,
    ) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.delete_trashed(paths).await?.try_into()?,
        })
    }
}
