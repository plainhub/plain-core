use super::{FileTaskResult, caller};
use plain_graphql::{Context, Object, Result};
use plain_rs::features::services::files::file_tasks::FileTasks;
use std::sync::Arc;

#[derive(Default)]
pub struct FileTaskQuery;

#[Object]
impl FileTaskQuery {
    async fn file_tasks(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> Result<Vec<FileTaskResult>> {
        let client_id = caller(ctx)?;
        crate::graphql::resolvers::common::validate_pagination(offset, limit)?;
        let text = query.to_lowercase();
        Ok(ctx
            .data::<Arc<FileTasks>>()?
            .list(client_id)
            .await?
            .into_iter()
            .filter(|t| t.title.to_lowercase().contains(&text))
            .skip(offset as usize)
            .take(limit as usize)
            .map(Into::into)
            .collect())
    }
}
