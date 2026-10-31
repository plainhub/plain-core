use crate::graphql::resolvers::packages::types::Certificate;
use crate::graphql::resolvers::packages::types::Package;
use crate::graphql::resolvers::packages::types::PackageInstallPending;
use crate::graphql::resolvers::packages::types::PackageStatus;
use crate::graphql::resolvers::packages::types::PackageType;
use crate::graphql::types::facts::flag;
use crate::graphql::types::facts::id;
use crate::graphql::types::facts::instant;
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::optional_instant;
use crate::graphql::types::facts::text;
use crate::http::auth::public_gate;
use async_graphql::{Context, Object};
use plain_rs::db::Db;
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use plain_rs::prefs::settings::FileSortBy;
use plain_rs::types::Long;
use serde_json::Value;
use std::sync::Arc;

const PERMISSION: &str = "QUERY_ALL_PACKAGES";

#[derive(Default)]
pub struct PackagesQuery;

#[Object]
impl PackagesQuery {
    async fn package_manifest(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<String> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[PERMISSION])?;
        plain_rs::services::capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "PACKAGES")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let statuses = ctx
            .data::<Arc<Host>>()?
            .call(
                "systemPackageStatuses",
                serde_json::json!({"ids":[id.as_str()]}),
            )
            .await
            .map_err(async_graphql::Error::new)?;
        let path = statuses
            .get(id.as_str())
            .and_then(|item| item.get("path"))
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .ok_or_else(|| async_graphql::Error::new("Package not found"))?;
        plain_rs::services::packages::read_manifest(ctx.data::<plain_rs::runtime::Runtime>()?, path)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }

    async fn packages(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: FileSortBy,
    ) -> async_graphql::Result<Vec<Package>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[PERMISSION])?;
        plain_rs::services::capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "PACKAGES")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let items = package_items(ctx, &query, sort_by).await?;
        Ok(items
            .into_iter()
            .skip(offset.max(0) as usize)
            .take(limit.max(0) as usize)
            .map(|item| package(&item))
            .collect())
    }

    async fn package_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[PERMISSION]).await? {
            return Ok(0);
        }
        plain_rs::services::capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "PACKAGES")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(package_items(ctx, &query, FileSortBy::NameAsc).await?.len() as i32)
    }

    async fn package_statuses(
        &self,
        ctx: &Context<'_>,
        ids: Vec<async_graphql::ID>,
    ) -> async_graphql::Result<Vec<PackageStatus>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[PERMISSION])?;
        plain_rs::services::capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "PACKAGES")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let ids: Vec<String> = ids.iter().map(|id| id.0.clone()).collect();
        let facts = ctx
            .data_unchecked::<Arc<Host>>()
            .call("systemPackageStatuses", serde_json::json!({ "ids": ids }))
            .await
            .map_err(|error| async_graphql::Error::new(error))?;
        Ok(package_statuses(&ids, &facts))
    }
}

#[derive(Default)]
pub struct PackagesMutation;

#[Object]
impl PackagesMutation {
    async fn uninstall_packages(
        &self,
        ctx: &Context<'_>,
        ids: Vec<async_graphql::ID>,
    ) -> async_graphql::Result<bool> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[PERMISSION])?;
        plain_rs::services::capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "PACKAGES")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let ids: Vec<String> = ids.iter().map(|id| id.0.clone()).collect();
        ctx.data_unchecked::<Arc<Host>>()
            .call("systemUninstallPackages", serde_json::json!({ "ids": ids }))
            .await
            .map_err(|error| async_graphql::Error::new(error))?;
        Ok(true)
    }

    async fn install_package(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<PackageInstallPending> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[PERMISSION])?;
        plain_rs::services::capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "PACKAGES")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let receipt = plain_rs::services::packages::install::execute(
            ctx.data::<plain_rs::runtime::Runtime>()?,
            ctx.data::<Arc<Host>>()?.clone(),
            &path,
        )
        .await
        .map_err(|error| async_graphql::Error::new(format!("Installation failed: {error}")))?;
        Ok(PackageInstallPending {
            id: async_graphql::ID::from(text(&receipt, "packageName")),
            updated_at: optional_instant(&receipt, "lastUpdateTime"),
            is_new: flag(&receipt, "isNew"),
        })
    }
}

async fn package_items(
    ctx: &Context<'_>,
    query: &str,
    sort_by: FileSortBy,
) -> async_graphql::Result<Vec<Value>> {
    let facts = ctx
        .data_unchecked::<Arc<Host>>()
        .call("systemPackageFacts", serde_json::json!({}))
        .await
        .map_err(|error| async_graphql::Error::new(error))?;
    let facts: Vec<Value> = serde_json::from_value(facts)
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    plain_rs::services::packages::query::packages(
        ctx.data_unchecked::<Arc<Db>>(),
        facts,
        query,
        sort_by.as_str(),
    )
    .map_err(|error| async_graphql::Error::new(error.to_string()))
}

fn package(value: &Value) -> Package {
    Package {
        id: id(value, "id"),
        name: text(value, "name"),
        r#type: match value["type"].as_str() {
            Some("SYSTEM") => PackageType::System,
            _ => PackageType::User,
        },
        version: text(value, "version"),
        path: text(value, "path"),
        size: Long(integer(value, "size")),
        certs: value["certs"]
            .as_array()
            .map(|certs| certs.iter().map(certificate).collect())
            .unwrap_or_default(),
        installed_at: instant(value, "installedAt"),
        updated_at: instant(value, "updatedAt"),
    }
}

fn certificate(value: &Value) -> Certificate {
    Certificate {
        issuer: text(value, "issuer"),
        subject: text(value, "subject"),
        serial_number: text(value, "serialNumber"),
        valid_from: instant(value, "validFrom"),
        valid_to: instant(value, "validTo"),
    }
}

/// One entry per requested id, in the order they were asked for: the host
/// answers with an object and its key order is not the caller's.
fn package_statuses(ids: &[String], facts: &Value) -> Vec<PackageStatus> {
    ids.iter()
        .map(|id| match facts.get(id) {
            None | Some(Value::Null) => PackageStatus {
                id: id.clone().into(),
                exists: false,
                updated_at: None,
            },
            Some(value) => PackageStatus {
                id: id.clone().into(),
                exists: true,
                updated_at: optional_instant(value, "updatedAt"),
            },
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/packages/public.rs"]
mod tests;
