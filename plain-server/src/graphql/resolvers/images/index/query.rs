//! Public `/graphql` image-search roots.
//!
//! The embedding model and the index itself are platform state; these roots
//! report it and forward the start/cancel/enable commands. Nothing here is
//! gated — plain-app lets any web client drive the index.

use plain_graphql::{Context, Object};
use plain_rs::features::services::images::status::ImageSearchStatus;
use std::sync::Arc;

#[derive(Default)]
pub struct ImageIndexQuery;

#[Object]
impl ImageIndexQuery {
    fn image_search_status(&self, ctx: &Context<'_>) -> plain_graphql::Result<ImageSearchStatus> {
        Ok(ctx
            .data_unchecked::<Arc<plain_rs::features::services::images::models::Runtime>>()
            .snapshot()
            .status)
    }
}
