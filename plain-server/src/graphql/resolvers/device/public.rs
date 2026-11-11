use crate::graphql::resolvers::app::types::Capability;
use crate::graphql::types::KeyValuePair;
use crate::graphql::types::facts::flag;
use plain_graphql::{ComplexObject, Context, Object, SimpleObject};

use plain_rs::features::access::AccessFeature;
use plain_rs::features::services::device::types::*;
use plain_rs::infrastructure::host::bridge::Host;
use plain_rs::infrastructure::types::Long;
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct App {
    pub client_id: String,
    pub http_port: i32,
    pub https_port: i32,
    pub device_name: String,
    pub developer_mode: bool,
    #[graphql(skip)]
    capabilities_sample: Arc<tokio::sync::OnceCell<Result<Vec<String>, String>>>,
    #[graphql(skip)]
    access_sample: Arc<tokio::sync::OnceCell<Result<Vec<AccessFeature>, String>>>,
}

impl App {
    async fn capability_names(&self, ctx: &Context<'_>) -> plain_graphql::Result<&Vec<String>> {
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
    async fn granted_access(&self, ctx: &Context<'_>) -> plain_graphql::Result<&Vec<AccessFeature>> {
        self.access_sample
            .get_or_init(|| async {
                let host = ctx.data::<Arc<Host>>().map_err(|error| error.message)?;
                plain_rs::features::services::device::access::snapshot_with_capabilities(
                    host,
                    self.capability_names(ctx)
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
        let mut names = self.capability_names(ctx).await?.clone();
        names.extend(
            plain_rs::features::services::files::browser::capabilities()
                .into_iter()
                .map(str::to_owned),
        );
        let mut seen = std::collections::HashSet::new();
        Ok(names
            .into_iter()
            .filter(|name| seen.insert(name.clone()))
            .filter_map(|name| Capability::parse(&name))
            .collect())
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
            client_id: system.client_id,
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

    /// Whether the mDNS advertiser is currently scanning. Starting and
    /// stopping are commands, not state, so they report success rather
    /// than a snapshot the caller would have to re-read.
    fn is_discovering(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        let facts = ctx.data::<crate::server::ServerState>()?.mdns.snapshot();
        Ok(flag(&facts, "scanning"))
    }
}

#[derive(Default)]
pub struct DeviceMutation;

#[Object]
impl DeviceMutation {
    async fn request_access(
        &self,
        ctx: &Context<'_>,
        feature: plain_rs::features::access::AccessFeature,
    ) -> plain_graphql::Result<bool> {
        if let Some(capability) = feature.capability() {
            plain_rs::features::services::capabilities::require(
                ctx.data_unchecked::<Arc<Host>>(),
                capability,
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        }
        let accepted = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemRequestAccess",
                plain_libs::serde_json::json!({
                    "feature": feature.as_str(),
                }),
            )
            .await
            .map_err(plain_graphql::Error::new)?;
        plain_libs::serde_json::from_value(accepted)
            .map_err(|error| plain_graphql::Error::new(error.to_string()))
    }

    async fn clear_app_logs(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        super::super::common::require_remote_developer_mode(ctx)?;
        ctx.data::<plain_rs::runtime::Runtime>()?
            .logs
            .clear()
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }

    async fn export_app_logs(&self, ctx: &Context<'_>) -> plain_graphql::Result<Option<String>> {
        super::super::common::require_remote_developer_mode(ctx)?;
        let state = ctx.data::<crate::server::ServerState>()?;
        ctx.data::<plain_rs::runtime::Runtime>()?
            .logs
            .export(&state.directory, &state.cache, state.stop.clone())
            .await
            .map_err(plain_graphql::Error::new)
    }

    fn set_temp_value(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: String,
    ) -> plain_graphql::Result<KeyValuePair> {
        ctx.data::<plain_rs::infrastructure::temp_values::TempValues>()?
            .set(key.clone(), value.clone());
        Ok(KeyValuePair { key, value })
    }

    /// The app restarts itself; the response only acknowledges that the
    /// request was accepted, since the connection dies with the process.
    async fn relaunch_app(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        plain_rs::features::services::capabilities::require(
            ctx.data::<Arc<Host>>()?,
            "APP_RELAUNCH",
        )
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
        host_call(ctx, "systemRelaunchApp", json!({})).await?;
        Ok(true)
    }

    /// Renaming the device also re-publishes the mDNS advertisement, so a
    /// client that waits for the new name to appear does see it.
    async fn update_device_name(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> plain_graphql::Result<bool> {
        crate::graphql::resolvers::settings::state::update(
            ctx,
            plain_rs::features::prefs::settings::SettingsPatch {
                device_name: Some(name),
                ..Default::default()
            },
        )
        .await?;
        Ok(true)
    }

    async fn start_discovery(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        plain_rs::features::services::discovery::mdns_runtime::control(
            ctx.data::<crate::server::ServerState>()?,
            plain_rs::features::services::discovery::mdns_runtime::Request::Start {},
        )
        .await?;
        Ok(true)
    }

    async fn stop_discovery(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        plain_rs::features::services::discovery::mdns_runtime::control(
            ctx.data::<crate::server::ServerState>()?,
            plain_rs::features::services::discovery::mdns_runtime::Request::Stop {},
        )
        .await?;
        Ok(true)
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

async fn host_call(ctx: &Context<'_>, method: &str, params: Value) -> plain_graphql::Result<Value> {
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(plain_graphql::Error::new)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/device/public.rs"]
mod tests;
