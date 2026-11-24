pub mod mutation;
pub mod sessions;

pub mod query;

use plain_graphql::{ComplexObject, Context, ID, SimpleObject};
use plain_rs::features::access::AccessFeature;
use plain_rs::features::capability::Capability;
use plain_rs::features::services::device::types::*;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct App {
    pub client_id: ID,
    pub http_port: i32,
    pub https_port: i32,
    pub device_name: String,
    pub developer_mode: bool,
    #[graphql(skip)]
    capabilities_sample: Arc<tokio::sync::OnceCell<Result<Vec<Capability>, String>>>,
    #[graphql(skip)]
    access_sample: Arc<tokio::sync::OnceCell<Result<Vec<AccessFeature>, String>>>,
}

impl App {
    async fn capability_sample(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<&Vec<Capability>> {
        self.capabilities_sample
            .get_or_init(|| async {
                plain_rs::features::services::capabilities::snapshot(
                    ctx.data::<Arc<Host>>().map_err(|error| error.message)?,
                )
                .await
                .map_err(|error| error.to_string())
            })
            .await
            .as_ref()
            .map_err(|error| plain_graphql::Error::new(error.clone()))
    }
    async fn granted_access(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<&Vec<AccessFeature>> {
        self.access_sample
            .get_or_init(|| async {
                let host = ctx.data::<Arc<Host>>().map_err(|error| error.message)?;
                plain_rs::features::services::device::access::snapshot_with_capabilities(
                    host,
                    self.capability_sample(ctx)
                        .await
                        .map_err(|error| error.message)?,
                )
                .await
            })
            .await
            .as_ref()
            .map_err(|error| plain_graphql::Error::new(error.clone()))
    }
}

#[ComplexObject]
impl App {
    fn url_token(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        Ok(crate::graphql::executor::resource_credentials(ctx)?.1)
    }

    async fn tls_certificate_sha256(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<Option<String>> {
        #[cfg(feature = "http_transport")]
        {
            let state = ctx.data::<crate::server::ServerState>()?;
            Ok(state
                .public_server
                .lock()
                .await
                .as_ref()
                .map(|server| server.tls_certificate_sha256.clone()))
        }
        #[cfg(not(feature = "http_transport"))]
        {
            let _ = ctx;
            Ok(None)
        }
    }
    fn app_dir(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        Ok(ctx
            .data::<plain_rs::runtime::Runtime>()?
            .device
            .directory()
            .to_string_lossy()
            .into_owned())
    }
    fn device_type(&self, ctx: &Context<'_>) -> plain_graphql::Result<DeviceType> {
        Ok(device_type(
            ctx.data::<Arc<Host>>()?.startup().device_type.as_str(),
        ))
    }
    async fn capabilities(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Capability>> {
        Ok(self.capability_sample(ctx).await?.clone())
    }
    fn build_channel(&self, ctx: &Context<'_>) -> plain_graphql::Result<AppChannelType> {
        Ok(app_channel(
            &ctx.data::<Arc<Host>>()?.startup().build_channel,
        ))
    }
    async fn access(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<AccessFeature>> {
        let grants = self.granted_access(ctx).await?;
        let names = if crate::http::auth::public_gate::is_local(ctx) {
            grants.clone()
        } else {
            plain_rs::features::services::device::access::enabled_features(
                ctx.data::<Arc<plain_rs::features::prefs::Prefs>>()?,
                grants,
            )
            .map_err(plain_graphql::Error::new)?
        };
        Ok(names)
    }
    fn downloads_dir(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        Ok(ctx.data::<Arc<Host>>()?.startup().downloads_dir.clone())
    }
    fn debug(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        Ok(ctx.data::<Arc<Host>>()?.startup().debug)
    }
}

pub(crate) fn device_type(value: &str) -> DeviceType {
    match value {
        "PHONE" => DeviceType::Phone,
        "TABLET" => DeviceType::Tablet,
        "TV" => DeviceType::Tv,
        "NAS" => DeviceType::Nas,
        "OTHER" => DeviceType::Other,
        _ => DeviceType::Computer,
    }
}

fn app_channel(value: &str) -> AppChannelType {
    match value {
        "GOOGLE" => AppChannelType::Google,
        "FDROID" => AppChannelType::Fdroid,
        _ => AppChannelType::Github,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/device/mod.rs"]
mod tests;
