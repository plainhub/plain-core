use plain_graphql::{Context, Object};
use std::sync::Arc;

#[derive(Default)]
pub struct ImageIndexMutation;

#[Object]
impl ImageIndexMutation {
    async fn enable_image_search(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::features::services::images::models::Runtime>>()
            .enable(false)
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }

    async fn import_image_search_model(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::features::services::images::models::Runtime>>()
            .import()
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }

    async fn disable_image_search(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::features::services::images::models::Runtime>>()
            .cancel(true)
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }

    async fn cancel_image_model_download(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::features::services::images::models::Runtime>>()
            .cancel(false)
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(true)
    }

    /// `force` re-scans images that already have an embedding.
    fn start_image_index(
        &self,
        ctx: &Context<'_>,
        force: Option<bool>,
    ) -> plain_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::features::services::images::index::ImageIndex>>()
            .start(force.unwrap_or(false))
            .map_err(|e| plain_graphql::Error::from(e))?;
        Ok(true)
    }

    fn cancel_image_index(&self, ctx: &Context<'_>) -> plain_graphql::Result<bool> {
        ctx.data_unchecked::<Arc<plain_rs::features::services::images::index::ImageIndex>>()
            .cancel()
            .map_err(|e| plain_graphql::Error::from(e))?;
        Ok(true)
    }
}
