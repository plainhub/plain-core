use plain_rs::features::access::AccessFeature;
use crate::graphql::types::ActionResult;
use crate::http::auth::public_gate;
use plain_graphql::{Context, ID, Object, Result};

use plain_rs::features::files::trash::TrashItem;
use plain_rs::features::services::files::file_tasks::FileTasks;
use std::sync::Arc;

struct TrashedFile {
    item: TrashItem,
    show_hidden: bool,
}

#[Object]
impl TrashedFile {
    async fn id(&self) -> ID {
        ID::from(self.item.id.as_str())
    }

    async fn file(&self, ctx: &Context<'_>) -> Result<super::types::File> {
        let platform = ctx.data::<Arc<plain_rs::features::services::files::platform::FilePlatform>>()?;
        Ok(plain_rs::features::services::files::browse::file_row(
            platform, self.item.path.clone(), self.item.name().to_owned(), self.show_hidden,
        ).await?.into())
    }
}

fn tasks<'a>(ctx: &'a Context<'_>) -> Result<&'a Arc<FileTasks>> {
    public_gate::require(
        ctx,
        ctx.data::<Arc<plain_rs::features::prefs::Prefs>>()?,
        &[AccessFeature::Files],
    )?;
    ctx.data::<Arc<FileTasks>>()
}
#[derive(Default)]
pub struct TrashQuery;
#[Object]
impl TrashQuery {
    async fn trashed_files(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        #[graphql(default)] show_hidden: bool,
    ) -> Result<Vec<TrashedFile>> {
        let rows = tasks(ctx)?.trashed(offset.try_into()?, limit.try_into()?, query).await?;
        if ctx.look_ahead().field("file").exists() && !rows.is_empty() {
            let paths = rows.iter().map(|row| row.path.to_string_lossy().into_owned()).collect::<Vec<_>>();
            ctx.data::<Arc<plain_rs::features::services::files::platform::FilePlatform>>()?
                .authorize_metadata_paths(&paths).await?;
        }
        Ok(rows.into_iter().map(|item| TrashedFile { item, show_hidden }).collect())
    }
    async fn trashed_file_count(&self, ctx: &Context<'_>, query: String) -> Result<i32> {
        Ok(tasks(ctx)?.trash_count(query).await?.try_into()?)
    }
}
#[derive(Default)]
pub struct TrashMutation;
#[Object]
impl TrashMutation {
    async fn trash_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.trash_paths(paths).await?.try_into()?,
        })
    }
    async fn restore_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult {
            affected_count: tasks(ctx)?.restore_paths(paths).await?.try_into()?,
        })
    }
    async fn delete_trashed_files(&self, ctx: &Context<'_>, paths: Vec<String>) -> Result<ActionResult> {
        Ok(ActionResult { affected_count: tasks(ctx)?.delete_trashed(paths).await?.try_into()? })
    }
    async fn delete_trashed_file(&self, ctx: &Context<'_>, path: String) -> Result<bool> {
        Ok(tasks(ctx)?.delete_trashed(vec![path]).await? != 0)
    }
}
