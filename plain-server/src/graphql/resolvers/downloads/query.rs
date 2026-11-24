use super::models::Download;
use crate::server::ServerState;
use plain_graphql::{Context, Object};

#[derive(Default)]
pub struct DownloadQuery;

#[Object]
impl DownloadQuery {
    fn downloads(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Download>> {
        Ok(ctx
            .data::<ServerState>()?
            .downloads
            .snapshot()
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
