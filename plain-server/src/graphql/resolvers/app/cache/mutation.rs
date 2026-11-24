use crate::server::ServerState;
use plain_graphql::{Context, Object};

#[derive(Default)]
pub struct CacheMutation;

#[Object]
impl CacheMutation {
    async fn clear_app_cache(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        let state = ctx.data::<ServerState>()?;
        plain_rs::infrastructure::cache::clear_application(state)
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }
}
