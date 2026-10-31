use crate::graphql::resolvers::app::types::Capability;
use crate::graphql::types::KeyValuePair;
use crate::graphql::types::facts::flag;
use async_graphql::{ComplexObject, Context, Object, SimpleObject};
use plain_rs::host::bridge::Host;
use plain_rs::services::device::permission::Permission;
use plain_rs::services::device::types::*;
use plain_rs::types::Long;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(SimpleObject, Clone)]
#[graphql(complex)]
pub struct App {
    pub client_id: String,
    /// Explicit read-only exception: authenticated clients need this existing
    /// file-access key. Generic prefs never expose it or allow writing it.
    pub url_token: String,
    pub http_port: i32,
    pub https_port: i32,
    pub device_name: String,
    pub developer_mode: bool,
    #[graphql(skip)]
    capabilities_sample: Arc<tokio::sync::OnceCell<Result<Vec<String>, String>>>,
    #[graphql(skip)]
    permissions_sample: Arc<tokio::sync::OnceCell<Result<Vec<String>, String>>>,
}

impl App {
    async fn granted_permissions(&self, ctx: &Context<'_>) -> async_graphql::Result<&Vec<String>> {
        self.permissions_sample
            .get_or_init(|| async {
                let host = ctx.data::<Arc<Host>>().map_err(|error| error.message)?;
                plain_rs::services::device::permissions::granted(host).await
            })
            .await
            .as_ref()
            .map_err(|error| async_graphql::Error::new(error.clone()))
    }
}

#[ComplexObject]
impl App {
    async fn tls_certificate_sha256(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<Option<String>> {
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
    async fn app_dir(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(ctx
            .data::<plain_rs::runtime::Runtime>()?
            .device
            .directory()
            .to_string_lossy()
            .into_owned())
    }
    async fn device_type(&self, ctx: &Context<'_>) -> async_graphql::Result<DeviceType> {
        Ok(device_type(
            ctx.data::<Arc<Host>>()?.startup().device_type.as_str(),
        ))
    }
    async fn capabilities(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Capability>> {
        let mut names = self
            .capabilities_sample
            .get_or_init(|| async {
                plain_rs::services::capabilities::snapshot(
                    ctx.data::<Arc<Host>>().map_err(|error| error.message)?,
                )
                .await
                .map_err(|error| error.to_string())
            })
            .await
            .as_ref()
            .map_err(|error| async_graphql::Error::new(error.clone()))?
            .clone();
        names.extend(
            plain_rs::services::files::browser::capabilities()
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
    async fn build_channel(&self, ctx: &Context<'_>) -> async_graphql::Result<AppChannelType> {
        Ok(app_channel(
            &ctx.data::<Arc<Host>>()?.startup().build_channel,
        ))
    }
    async fn permissions(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Permission>> {
        let grants = self.granted_permissions(ctx).await?;
        let names = if crate::http::auth::public_gate::is_local(ctx) {
            grants.clone()
        } else {
            plain_rs::services::device::permissions::enabled(
                ctx.data::<Arc<plain_rs::prefs::Prefs>>()?,
                grants,
            )
            .map_err(async_graphql::Error::new)?
        };
        Ok(names
            .iter()
            .filter_map(|name| Permission::parse(name))
            .collect())
    }
    async fn downloads_dir(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(ctx.data::<Arc<Host>>()?.startup().downloads_dir.clone())
    }
    async fn debug(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        Ok(ctx.data::<Arc<Host>>()?.startup().debug)
    }
}

#[derive(Default)]
pub struct DeviceQuery;

#[Object]
impl DeviceQuery {
    async fn device_status(&self, ctx: &Context<'_>) -> async_graphql::Result<DeviceStatus> {
        let fields = ctx.look_ahead();
        ctx.data::<plain_rs::runtime::Runtime>()?
            .device
            .collect(
                ctx.data::<Arc<Host>>()?,
                plain_rs::services::device::Selection {
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
            .map_err(async_graphql::Error::new)
    }

    async fn device_info(&self, ctx: &Context<'_>) -> async_graphql::Result<DeviceInfo> {
        let look = ctx.look_ahead();
        let mut fields: Vec<String> = plain_rs::services::device::info::FIELDS
            .iter()
            .filter(|name| look.field(name).exists())
            .map(|name| (*name).to_owned())
            .collect();
        for (name, children) in [
            ("display", plain_rs::services::device::info::DISPLAY_FIELDS),
            ("android", plain_rs::services::device::info::ANDROID_FIELDS),
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
        plain_rs::services::device::info::collect(
            ctx.data::<Arc<Host>>()?,
            ctx.data::<plain_rs::runtime::Runtime>()?
                .device
                .directory()
                .to_path_buf(),
            fields,
            ctx.data::<plain_rs::runtime::Runtime>()?
                .device
                .fixed_info()
                .map_err(async_graphql::Error::new)?,
        )
        .await
        .map_err(async_graphql::Error::new)
    }

    /// Everything a client needs to reach this device: its identity, the
    /// port it listens on and the token that authorises it.
    async fn app(&self, ctx: &Context<'_>) -> async_graphql::Result<App> {
        let prefs = ctx.data::<Arc<plain_rs::prefs::Prefs>>()?;
        let user = plain_rs::prefs::settings::snapshot(prefs)?;
        let system = plain_rs::prefs::security::snapshot(prefs)?;
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
            url_token: system.url_token,
            http_port,
            https_port,
            device_name: user.device_name,
            developer_mode: user.developer_mode,
            capabilities_sample: Arc::new(tokio::sync::OnceCell::new()),
            permissions_sample: Arc::new(tokio::sync::OnceCell::new()),
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
    ) -> async_graphql::Result<Vec<String>> {
        ctx.data::<plain_rs::runtime::Runtime>()?
            .logs
            .read(text_of(&query), offset, limit)
            .await
            .map_err(async_graphql::Error::new)
    }

    async fn app_log_files_size(&self, ctx: &Context<'_>) -> async_graphql::Result<Long> {
        let size = ctx
            .data::<plain_rs::runtime::Runtime>()?
            .logs
            .size()
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(Long(i64::try_from(size)?))
    }

    async fn app_log_path(&self, ctx: &Context<'_>) -> async_graphql::Result<String> {
        Ok(ctx
            .data::<plain_rs::runtime::Runtime>()?
            .logs
            .path()
            .map_err(async_graphql::Error::new)?
            .to_string_lossy()
            .into_owned())
    }

    /// Whether the mDNS advertiser is currently scanning. Starting and
    /// stopping are commands, not state, so they report success rather
    /// than a snapshot the caller would have to re-read.
    async fn is_discovering(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        let facts = ctx.data::<crate::server::ServerState>()?.mdns.snapshot();
        Ok(flag(&facts, "scanning"))
    }
}

#[derive(Default)]
pub struct DeviceMutation;

#[Object]
impl DeviceMutation {
    async fn clear_app_logs(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data::<plain_rs::runtime::Runtime>()?
            .logs
            .clear()
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }

    async fn export_app_logs(&self, ctx: &Context<'_>) -> async_graphql::Result<Option<String>> {
        let state = ctx.data::<crate::server::ServerState>()?;
        ctx.data::<plain_rs::runtime::Runtime>()?
            .logs
            .export(&state.directory, &state.cache, state.stop.clone())
            .await
            .map_err(async_graphql::Error::new)
    }

    async fn set_temp_value(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: String,
    ) -> async_graphql::Result<KeyValuePair> {
        ctx.data::<plain_rs::services::temp_values::TempValues>()?
            .set(key.clone(), value.clone());
        Ok(KeyValuePair { key, value })
    }

    /// The app restarts itself; the response only acknowledges that the
    /// request was accepted, since the connection dies with the process.
    async fn relaunch_app(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        plain_rs::services::capabilities::require(ctx.data::<Arc<Host>>()?, "APP_RELAUNCH")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        host_call(ctx, "systemRelaunchApp", json!({})).await?;
        Ok(true)
    }

    /// Renaming the device also re-publishes the mDNS advertisement, so a
    /// client that waits for the new name to appear does see it.
    async fn update_device_name(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> async_graphql::Result<bool> {
        crate::graphql::resolvers::settings::state::update(
            ctx,
            plain_rs::prefs::settings::SettingsPatch {
                device_name: Some(name),
                ..Default::default()
            },
        )
        .await?;
        Ok(true)
    }

    async fn start_discovery(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        plain_rs::services::discovery::mdns_runtime::control(
            ctx.data::<crate::server::ServerState>()?,
            plain_rs::services::discovery::mdns_runtime::Request::Start {},
        )
        .await?;
        Ok(true)
    }

    async fn stop_discovery(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        plain_rs::services::discovery::mdns_runtime::control(
            ctx.data::<crate::server::ServerState>()?,
            plain_rs::services::discovery::mdns_runtime::Request::Stop {},
        )
        .await?;
        Ok(true)
    }
}

/// The `text` field of the search DSL, which is what the log view filters
/// on. A query with other fields but no `text` still filters on nothing.
fn text_of(query: &str) -> String {
    plain_rs::utils::search_dsl::parse(query)
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

async fn host_call(ctx: &Context<'_>, method: &str, params: Value) -> async_graphql::Result<Value> {
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(async_graphql::Error::new)
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/device/public.rs"]
mod tests;
