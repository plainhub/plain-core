use super::FEATURE;
use crate::graphql::resolvers::packages::types::PackageInstallPending;
use crate::graphql::types::facts::{flag, optional_instant, text};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct PackagesMutation;

#[Object]
impl PackagesMutation {
    /// Launch system uninstall requests; true means initiated, not uninstalled. Confirm removal with packageStatuses.
    async fn uninstall_packages(
        &self,
        ctx: &Context<'_>,
        ids: Vec<plain_graphql::ID>,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::Packages,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let ids: Vec<String> = ids.iter().map(|id| id.0.clone()).collect();
        ctx.data_unchecked::<Arc<Host>>()
            .call(
                "systemUninstallPackages",
                plain_libs::serde_json::json!({ "ids": ids }),
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        Ok(true)
    }

    async fn install_package(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> plain_graphql::Result<PackageInstallPending> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        plain_rs::features::services::capabilities::require(
            ctx.data_unchecked::<Arc<Host>>(),
            Capability::Packages,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        let receipt = plain_rs::features::services::packages::install::execute(
            ctx.data::<plain_rs::runtime::Runtime>()?,
            ctx.data::<Arc<Host>>()?.clone(),
            &path,
        )
        .await
        .map_err(|error| plain_graphql::Error::new(format!("Installation failed: {error}")))?;
        Ok(PackageInstallPending {
            id: plain_graphql::ID::from(text(&receipt, "packageName")),
            updated_at: optional_instant(&receipt, "lastUpdateTime"),
            is_new: flag(&receipt, "isNew"),
        })
    }
}
