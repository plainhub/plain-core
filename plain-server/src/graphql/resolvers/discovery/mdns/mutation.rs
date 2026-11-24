use crate::server::ServerState;
use plain_graphql::{Context, Object};
use plain_rs::features::services::discovery::mdns_runtime;

#[derive(Default)]
pub struct MdnsMutation;

#[Object]
impl MdnsMutation {
    async fn acquire_mdns_debug_session(
        &self,
        ctx: &Context<'_>,
        token: String,
    ) -> plain_graphql::Result<bool> {
        crate::graphql::resolvers::app::http_server::authorized_capabilities(ctx).await?;
        mdns_runtime::control(
            ctx.data::<ServerState>()?,
            mdns_runtime::Request::DebugStart { token },
        )
        .await?;
        Ok(true)
    }

    async fn release_mdns_debug_session(
        &self,
        ctx: &Context<'_>,
        token: String,
    ) -> plain_graphql::Result<bool> {
        crate::graphql::resolvers::app::http_server::authorized_capabilities(ctx).await?;
        mdns_runtime::control(
            ctx.data::<ServerState>()?,
            mdns_runtime::Request::DebugStop { token },
        )
        .await?;
        Ok(true)
    }

    async fn start_discovery(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        mdns_runtime::control(ctx.data::<ServerState>()?, mdns_runtime::Request::Start {}).await?;
        Ok(true)
    }

    async fn stop_discovery(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        mdns_runtime::control(ctx.data::<ServerState>()?, mdns_runtime::Request::Stop {}).await?;
        Ok(true)
    }
}
