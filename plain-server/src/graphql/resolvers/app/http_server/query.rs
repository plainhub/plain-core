use super::{HttpServer, authorized_capabilities, snapshot};
use crate::server::ServerState;
use plain_graphql::{Context, Object, Result};

#[derive(Default)]
pub struct HttpServerQuery;

#[Object]
impl HttpServerQuery {
    async fn http_server(&self, ctx: &Context<'_>) -> Result<HttpServer> {
        authorized_capabilities(ctx).await?;
        snapshot(ctx.data::<ServerState>()?).await
    }
}
