use super::App;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::services::device::types::*;
use plain_rs::infrastructure::host::bridge::Host;
use plain_rs::infrastructure::types::Long;
use std::sync::Arc;

#[derive(Default)]
pub struct DeviceQuery;

#[Object]
impl DeviceQuery {
    async fn device_status(&self, ctx: &Context<'_>) -> plain_graphql::Result<DeviceStatus> {
        let fields = ctx.look_ahead();
        ctx.data::<plain_rs::runtime::Runtime>()?
            .device
            .collect(
                ctx.data::<Arc<Host>>()?,
                plain_rs::features::services::device::Selection {
                    uptime: fields.field("uptimeSec").exists(),
                    battery: fields.field("batteryLevel").exists()
                        || fields.field("charging").exists(),
                    temperatures: fields.field("temperatures").exists(),
                    cpu: fields.field("cpuUsage").exists(),
                    memory: fields.field("memoryAvailable").exists(),
                    storage: fields.field("storageAvailable").exists(),
                },
            )
            .await
            .map_err(plain_graphql::Error::new)
    }

    async fn device_info(&self, ctx: &Context<'_>) -> plain_graphql::Result<DeviceInfo> {
        let look = ctx.look_ahead();
        let mut fields: Vec<String> = plain_rs::features::services::device::info::FIELDS
            .iter()
            .filter(|name| look.field(name).exists())
            .map(|name| (*name).to_owned())
            .collect();
        for (name, children) in [
            (
                "display",
                plain_rs::features::services::device::info::DISPLAY_FIELDS,
            ),
            (
                "android",
                plain_rs::features::services::device::info::ANDROID_FIELDS,
            ),
        ] {
            if look.field(name).exists() {
                fields.push(name.into());
            }
            for child in children {
                if look.field(name).field(child).exists() {
                    fields.push(format!("{name}.{child}"));
                }
            }
        }
        plain_rs::features::services::device::info::collect(
            ctx.data::<Arc<Host>>()?,
            ctx.data::<plain_rs::runtime::Runtime>()?
                .device
                .directory()
                .to_path_buf(),
            fields,
            ctx.data::<plain_rs::runtime::Runtime>()?
                .device
                .fixed_info()
                .map_err(plain_graphql::Error::new)?,
        )
        .await
        .map_err(plain_graphql::Error::new)
    }

    /// Everything a client needs to reach this device: its identity, the
    /// port it listens on and the token that authorises it.
    async fn app(&self, ctx: &Context<'_>) -> plain_graphql::Result<App> {
        let prefs = ctx.data::<Arc<plain_rs::features::prefs::Prefs>>()?;
        let user = plain_rs::features::prefs::settings::snapshot(prefs)?;
        let system = plain_rs::features::prefs::security::snapshot(prefs)?;
        let (http_port, https_port) = match ctx.data_opt::<crate::server::ServerState>() {
            Some(state) => state
                .public_ports()
                .await
                .map(|(http, https)| (i32::from(http), i32::from(https)))
                .unwrap_or((0, 0)),
            None => (user.http_port, user.https_port),
        };
        Ok(App {
            client_id: ID(system.client_id),
            http_port,
            https_port,
            device_name: user.device_name,
            developer_mode: user.developer_mode,
            capabilities_sample: Arc::new(tokio::sync::OnceCell::new()),
            access_sample: Arc::new(tokio::sync::OnceCell::new()),
        })
    }

    /// Log lines, newest first. A blank `query` pages the buffer directly;
    /// a non-blank one filters the whole buffer and then pages, so a
    /// search cannot be cut short by the page size.
    async fn app_logs(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<String>> {
        super::super::common::require_remote_developer_mode(ctx)?;
        ctx.data::<plain_rs::runtime::Runtime>()?
            .logs
            .read(text_of(&query), offset, limit)
            .await
            .map_err(plain_graphql::Error::new)
    }

    async fn app_log_files_size(&self, ctx: &Context<'_>) -> plain_graphql::Result<Long> {
        super::super::common::require_remote_developer_mode(ctx)?;
        let size = ctx
            .data::<plain_rs::runtime::Runtime>()?
            .logs
            .size()
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(Long(i64::try_from(size)?))
    }

    fn app_log_path(&self, ctx: &Context<'_>) -> plain_graphql::Result<String> {
        super::super::common::require_remote_developer_mode(ctx)?;
        Ok(ctx
            .data::<plain_rs::runtime::Runtime>()?
            .logs
            .path()
            .map_err(plain_graphql::Error::new)?
            .to_string_lossy()
            .into_owned())
    }
}

/// The `text` field of the search DSL, which is what the log view filters
/// on. A query with other fields but no `text` still filters on nothing.
fn text_of(query: &str) -> String {
    plain_libs::utils::search_dsl::parse(query)
        .into_iter()
        .find(|field| field.name == "text")
        .map(|field| field.value)
        .unwrap_or_default()
}
