use crate::graphql::resolvers::files::app_file::AppFile;
use async_graphql::{Context, Object};
use plain_rs::files::app_files::FileStore;
use std::sync::Arc;

#[derive(Default)]
pub struct AppFilesMutation;
#[Object]
impl AppFilesMutation {
    async fn import_app_file(
        &self,
        ctx: &Context<'_>,
        source: String,
        file_name: String,
        mime_type: String,
        delete_source: bool,
    ) -> async_graphql::Result<AppFile> {
        crate::http::auth::public_gate::require(
            ctx,
            ctx.data::<Arc<plain_rs::prefs::Prefs>>()?,
            &["WRITE_EXTERNAL_STORAGE"],
        )?;
        let store = ctx.data::<Arc<FileStore>>()?;
        let state = ctx.data::<crate::server::ServerState>()?;
        let file = plain_rs::services::files::imports::import(
            store,
            &state.resources,
            state.cache.root(),
            plain_rs::services::files::imports::Import {
                source,
                name: file_name.clone(),
                mime: mime_type,
                delete_source,
            },
            state.stop.clone(),
            state.cache_changes.clone(),
        )
        .await
        .map_err(async_graphql::Error::new)?;
        let name = if file_name.is_empty() {
            plain_rs::chat::attachments::app_file_store::display_name(&file, &Default::default())
        } else {
            file_name
        };
        Ok(AppFile::from_dappfile(file, name))
    }
    async fn release_app_file(
        &self,
        ctx: &Context<'_>,
        file_id: String,
    ) -> async_graphql::Result<bool> {
        ctx.data::<Arc<FileStore>>()?
            .release(file_id)
            .await
            .map_err(async_graphql::Error::new)
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/files/app_files.rs"]
mod tests;
