use crate::graphql::types::KeyValuePair;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::{Value, json};
use plain_rs::features::capability::Capability;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

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
            .map_err(|error| plain_graphql::Error::from(error))?;
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
            .map_err(|error| plain_graphql::Error::from(error))
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
            Capability::AppRelaunch,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
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
}

async fn host_call(ctx: &Context<'_>, method: &str, params: Value) -> plain_graphql::Result<Value> {
    ctx.data_unchecked::<Arc<Host>>()
        .call(method, params)
        .await
        .map_err(plain_graphql::Error::new)
}
