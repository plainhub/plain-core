use super::{Share, record, state};
use plain_graphql::{Context, ID, Object};

#[derive(Default)]
pub struct ShareHostQuery;

#[Object]
impl ShareHostQuery {
    async fn share(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<Option<Share>> {
        let state = state(ctx)?;
        match state.db.share_get(&id)? {
            Some(row) => Ok(Some(record(state, row).await?)),
            None => Ok(None),
        }
    }
    async fn shares(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Share>> {
        let state = state(ctx)?;
        let mut shares = vec![];
        for row in state.db.share_list()? {
            shares.push(record(state, row).await?);
        }
        Ok(shares)
    }
}
