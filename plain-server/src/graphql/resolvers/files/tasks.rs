use async_graphql::{Context, ID, InputObject, Object, Result, SimpleObject};
use plain_rs::files::tasks::CompletedOp;
use plain_rs::files::tasks::FileTask;
use plain_rs::files::tasks::FileTaskOp;
use plain_rs::files::tasks::FileTaskStatus;
use plain_rs::files::tasks::FileTaskType;
use plain_rs::services::files::file_tasks::FileTasks;
use plain_rs::types::Instant;
use plain_rs::types::Long;
use std::sync::Arc;
#[derive(InputObject)]
struct FileTaskOpInput {
    src: String,
    dst: String,
    overwrite: bool,
}
#[derive(SimpleObject)]
struct FileTaskCompletedOp {
    src: String,
    dst: String,
}
impl From<CompletedOp> for FileTaskCompletedOp {
    fn from(op: CompletedOp) -> Self {
        Self {
            src: op.src,
            dst: op.dst,
        }
    }
}
#[derive(SimpleObject)]
#[graphql(name = "FileTask")]
struct FileTaskResult {
    id: ID,
    client_id: String,
    r#type: FileTaskType,
    title: String,
    status: FileTaskStatus,
    error: String,
    total_bytes: Long,
    done_bytes: Long,
    total_items: Long,
    done_items: Long,
    created_at: Instant,
    updated_at: Instant,
    completed_ops: Vec<FileTaskCompletedOp>,
}
impl From<FileTask> for FileTaskResult {
    fn from(t: FileTask) -> Self {
        Self {
            id: ID(t.id),
            client_id: t.client_id,
            r#type: t.kind,
            title: t.title,
            status: t.status,
            error: t.error,
            total_bytes: Long(t.total_bytes),
            done_bytes: Long(t.done_bytes),
            total_items: Long(t.total_items),
            done_items: Long(t.done_items),
            created_at: Instant(t.created_at),
            updated_at: Instant(t.updated_at),
            completed_ops: t.completed_ops.into_iter().map(Into::into).collect(),
        }
    }
}
#[derive(Default)]
pub struct FileTaskQuery;
#[Object]
impl FileTaskQuery {
    async fn file_tasks(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> Result<Vec<FileTaskResult>> {
        let client_id = caller(ctx)?;
        if offset < 0 || limit < 0 {
            return Err("invalid pagination".into());
        }
        let text = query.to_lowercase();
        Ok(ctx
            .data::<Arc<FileTasks>>()?
            .list(client_id)
            .await?
            .into_iter()
            .filter(|t| t.title.to_lowercase().contains(&text))
            .skip(offset as usize)
            .take(limit as usize)
            .map(Into::into)
            .collect())
    }
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

pub(super) fn caller(ctx: &Context<'_>) -> Result<String> {
    let client_id = ctx.data::<String>()?;
    if client_id.is_empty() {
        return Err("unauthorized".into());
    }
    Ok(client_id.clone())
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
        ctx.data::<Arc<plain_rs::prefs::Prefs>>()?,
        &["WRITE_EXTERNAL_STORAGE"],
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
