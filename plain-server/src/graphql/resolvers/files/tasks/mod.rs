pub mod mutation;
pub mod query;

use plain_graphql::{Context, ID, Result, SimpleObject};
use plain_rs::features::access_error::AccessErrorCode;
use plain_rs::features::files::tasks::{CompletedOp, FileTask, FileTaskStatus, FileTaskType};
use plain_rs::infrastructure::types::{Instant, Long};

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
    client_id: ID,
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
            client_id: ID(t.client_id),
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

pub(super) fn caller(ctx: &Context<'_>) -> Result<String> {
    let client_id = ctx.data::<String>()?;
    if client_id.is_empty() {
        return Err(AccessErrorCode::Unauthorized.into());
    }
    Ok(client_id.clone())
}
