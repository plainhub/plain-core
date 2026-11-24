use super::{FileTaskResult, caller};
use plain_graphql::{Context, InputObject, Object, Result};
use plain_rs::features::access::AccessFeature;
use plain_rs::features::files::tasks::{FileTaskOp, FileTaskType};
use plain_rs::features::services::files::file_tasks::FileTasks;
use std::sync::Arc;

#[derive(InputObject)]
struct FileTaskOpInput {
    src: String,
    dst: String,
    overwrite: bool,
}

#[derive(Default)]
pub struct FileTaskMutation;

#[Object]
impl FileTaskMutation {
    async fn create_file_task(
        &self,
        ctx: &Context<'_>,
        r#type: FileTaskType,
        title: String,
        ops: Vec<FileTaskOpInput>,
        #[graphql(default)] wait_for_completion: bool,
    ) -> Result<FileTaskResult> {
        create(ctx, r#type, title, ops, wait_for_completion).await
    }
}

async fn create(
    ctx: &Context<'_>,
    r#type: FileTaskType,
    title: String,
    ops: Vec<FileTaskOpInput>,
    wait_for_completion: bool,
) -> Result<FileTaskResult> {
    crate::http::auth::public_gate::require(
        ctx,
        ctx.data::<Arc<plain_rs::features::prefs::Prefs>>()?,
        &[AccessFeature::Files],
    )?;
    let client_id = caller(ctx)?;
    let tasks = ctx.data::<Arc<FileTasks>>()?;
    let ops = ops
        .into_iter()
        .map(|op| FileTaskOp {
            src: op.src,
            dst: op.dst,
            overwrite: op.overwrite,
        })
        .collect();
    let task = if wait_for_completion {
        tasks
            .create_and_wait(&client_id, r#type, &title, ops)
            .await?
    } else {
        tasks.create(&client_id, r#type, &title, ops).await?
    };
    Ok(task.into())
}
