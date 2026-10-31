//! Public `/graphql` image-search roots.
//!
//! The embedding model and the index itself are platform state; these roots
//! report it and forward the start/cancel/enable commands. Nothing here is
//! gated — plain-app lets any web client drive the index.

use plain_rs::services::images::status::ImageSearchStatus;

use async_graphql::{Context, Object};
use std::sync::Arc;

#[derive(Default)]
pub struct ImageIndexQuery;

#[Object]
impl ImageIndexQuery {
    async fn image_search_status(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<ImageSearchStatus> {
        Ok(ctx
            .data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>()
            .snapshot()
            .status)
    }
}

#[derive(Default)]
pub struct ImageIndexMutation;

#[Object]
impl ImageIndexMutation {
    async fn enable_image_search(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>()
            .enable(false)
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }

    async fn import_image_search_model(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>()
            .import()
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }

    async fn disable_image_search(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>()
            .cancel(true)
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }

    async fn cancel_image_model_download(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::services::images::models::Runtime>>()
            .cancel(false)
            .await
            .map_err(async_graphql::Error::new)?;
        Ok(true)
    }

    /// `force` re-scans images that already have an embedding.
    async fn start_image_index(
        &self,
        ctx: &Context<'_>,
        force: Option<bool>,
    ) -> async_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::services::images::index::ImageIndex>>()
            .start(force.unwrap_or(false))
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(true)
    }

    async fn cancel_image_index(&self, ctx: &Context<'_>) -> async_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::services::images::index::ImageIndex>>()
            .cancel()
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/images/public_index.rs"]
mod tests;
