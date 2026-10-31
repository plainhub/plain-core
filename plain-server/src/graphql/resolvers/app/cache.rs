use crate::server::ServerState;
use async_graphql::{Context, Object};
use plain_rs::types::Long;

#[derive(Default)]
pub struct CacheQuery;
#[Object]
impl CacheQuery {
    async fn app_cache_size(&self, ctx: &Context<'_>) -> async_graphql::Result<Long> {
        let cache = ctx.data::<ServerState>()?.cache.clone();
        let bytes =
            tokio::task::spawn_blocking(move || plain_rs::services::cache::size(&cache)).await??;
        Ok(Long(i64::try_from(bytes)?))
    }
}

#[derive(Default)]
pub struct CacheMutation;
#[Object]
impl CacheMutation {
    async fn clear_app_cache(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        let state = ctx.data::<ServerState>()?;
        plain_rs::services::cache::clear_application(state)
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }
}
