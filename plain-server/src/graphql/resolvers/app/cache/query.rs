use crate::server::ServerState;
use plain_graphql::{Context, Object};
use plain_rs::infrastructure::types::Long;

#[derive(Default)]
pub struct CacheQuery;

#[Object]
impl CacheQuery {
    async fn app_cache_size(&self, ctx: &Context<'_>) -> plain_graphql::Result<Long> {
        let cache = ctx.data::<ServerState>()?.cache.clone();
        let bytes =
            tokio::task::spawn_blocking(move || plain_rs::infrastructure::cache::size(&cache))
                .await??;
        Ok(Long(i64::try_from(bytes)?))
    }
}
