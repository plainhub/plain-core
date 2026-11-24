use super::tasks;
use plain_graphql::{Context, Object, Result};

#[derive(Default)]
pub struct TrashQuery;

#[Object]
impl TrashQuery {
    async fn trashed_files(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> Result<Vec<super::super::types::File>> {
        let rows = tasks(ctx)?
            .trashed(offset.try_into()?, limit.try_into()?, query)
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
    async fn trashed_file_count(&self, ctx: &Context<'_>, query: String) -> Result<i32> {
        Ok(tasks(ctx)?.trash_count(query).await?.try_into()?)
    }
}
