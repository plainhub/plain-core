use plain_graphql::{Context, Object};

use plain_rs::features::files::app_files::FileStore;
use std::sync::Arc;

#[derive(Default)]
pub struct AppFilesMutation;
#[Object]
impl AppFilesMutation {
    async fn release_app_file(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> plain_graphql::Result<bool> {
        ctx.data::<Arc<FileStore>>()?
            .release(file_id)
            .await
            .map_err(plain_graphql::Error::new)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/files/app_files.rs"]
mod tests;
