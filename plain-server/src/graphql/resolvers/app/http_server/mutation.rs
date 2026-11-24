use super::authorized_capabilities;
use crate::server::ServerState;
use plain_graphql::{Context, Object, Result};
use plain_rs::features::services::http_status::HttpServerState;

#[derive(Default)]
pub struct HttpServerMutation;

#[Object]
impl HttpServerMutation {
    async fn restart_http_server(
        &self,
        ctx: &Context<'_>,
        http_port: Option<i32>,
        https_port: Option<i32>,
    ) -> Result<HttpServerState> {
        authorized_capabilities(ctx).await?;
        let port = |value: i32| {
            u16::try_from(value).map_err(|_| plain_graphql::Error::new("Invalid server port"))
        };
        let http = http_port.map(port).transpose()?;
        let https = https_port.map(port).transpose()?;
        crate::http::listeners::control::restart_and_wait(ctx.data::<ServerState>()?, http, https)
            .await
            .map_err(plain_graphql::Error::new)
    }
}
