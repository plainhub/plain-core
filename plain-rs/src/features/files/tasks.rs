use plain_libs::anyhow::{Result, anyhow, bail};
use plain_libs::chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum FileTaskType {
    Copy,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum FileTaskStatus {
    Queued,
    Running,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTaskOp {
    pub src: String,
    pub dst: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedOp {
    pub src: String,
    pub dst: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTask {
    pub id: String,
    pub client_id: String,
    #[serde(rename = "type")]
    pub kind: FileTaskType,
    pub title: String,
    pub status: FileTaskStatus,
    pub error: String,
    pub total_bytes: i64,
    pub done_bytes: i64,
    pub total_items: i64,
    pub done_items: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_ops: Vec<CompletedOp>,
}
pub trait Events: Send + Sync + 'static {
    fn changed(&self, task: &FileTask);
}
pub type HookResult<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
pub type PrepareResult<'a> = Pin<Box<dyn Future<Output = Result<plain_libs::serde_json::Value>> + Send + 'a>>;
pub trait Hooks: Send + Sync + 'static {
    fn prepare<'a>(&'a self, _: FileTaskType, _: &'a FileTaskOp) -> PrepareResult<'a> {
        Box::pin(async { Ok(plain_libs::serde_json::Value::Null) })
    }
    fn completed_with_snapshot<'a>(
        &'a self,
        kind: FileTaskType,
        op: &'a CompletedOp,
        _: &'a plain_libs::serde_json::Value,
    ) -> HookResult<'a> {
        self.completed(kind, op)
    }
    fn authorize<'a>(&'a self, kind: FileTaskType, ops: &'a [FileTaskOp]) -> HookResult<'a>;
    fn completed<'a>(&'a self, kind: FileTaskType, op: &'a CompletedOp) -> HookResult<'a>;
}
struct NoHooks;
impl Hooks for NoHooks {
    fn authorize<'a>(&'a self, _: FileTaskType, _: &'a [FileTaskOp]) -> HookResult<'a> {
        Box::pin(async { Ok(()) })
    }
    fn completed<'a>(&'a self, _: FileTaskType, _: &'a CompletedOp) -> HookResult<'a> {
        Box::pin(async { Ok(()) })
    }
}
struct Request {
    task: FileTask,
    ops: Vec<FileTaskOp>,
    rename: bool,
}
type Tasks = Arc<Mutex<HashMap<String, FileTask>>>;
pub struct Service {
    queue: mpsc::Sender<Request>,
    tasks: Tasks,
    events: Arc<dyn Events>,
}
impl Service {
    pub fn new(events: Arc<dyn Events>) -> Self {
        Self::with_hooks(events, Arc::new(NoHooks))
    }
    pub fn with_hooks(events: Arc<dyn Events>, hooks: Arc<dyn Hooks>) -> Self {
        let (queue, mut receiver) = mpsc::channel::<Request>(32);
        let tasks = Tasks::default();
        let state = tasks.clone();
        let emitter = events.clone();
        tokio::spawn(async move {
            while let Some(request) = receiver.recv().await {
                execute(request, &state, &emitter, &hooks).await;
            }
        });
        Self {
            queue,
            tasks,
            events,
        }
    }
    pub fn create(
        &self,
        client_id: &str,
        kind: FileTaskType,
        title: &str,
        ops: Vec<FileTaskOp>,
    ) -> Result<FileTask> {
        self.admit(client_id, kind, title, ops, false)
    }
    pub fn rename(&self, client_id: &str, src: String, dst: String) -> Result<FileTask> {
        self.admit(
            client_id,
            FileTaskType::Move,
            "rename",
            vec![FileTaskOp {
                src,
                dst,
                overwrite: false,
            }],
            true,
        )
    }
    fn admit(
        &self,
        client_id: &str,
        kind: FileTaskType,
        title: &str,
        ops: Vec<FileTaskOp>,
        rename: bool,
    ) -> Result<FileTask> {
        if client_id.is_empty() {
            bail!("unauthorized");
        }
        if ops.is_empty() {
            bail!("no operations");
        }
        let permit = self
            .queue
            .try_reserve()
            .map_err(|e| anyhow!("file task queue: {e}"))?;
        let now = Utc::now();
        let task = FileTask {
            id: plain_libs::utils::uuid::Uuid::new_v4().into_string(),
            client_id: client_id.into(),
            kind,
            title: title.into(),
            status: FileTaskStatus::Queued,
            error: String::new(),
            total_bytes: 0,
            done_bytes: 0,
            total_items: 0,
            done_items: 0,
            created_at: now,
            updated_at: now,
            completed_ops: Vec::new(),
        };
        save(&task, &self.tasks, &self.events)?;
        permit.send(Request {
            task: task.clone(),
            ops,
            rename,
        });
        Ok(task)
    }
    pub fn get(&self, id: &str) -> Result<Option<FileTask>> {
        Ok(self
            .tasks
            .lock()
            .map_err(|_| anyhow!("file task state poisoned"))?
            .get(id)
            .cloned())
    }
    pub fn list(&self, client_id: &str) -> Result<Vec<FileTask>> {
        if client_id.is_empty() {
            bail!("unauthorized");
        }
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| anyhow!("file task state poisoned"))?
            .values()
            .filter(|task| task.client_id == client_id)
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(tasks)
    }
}
fn save(task: &FileTask, tasks: &Tasks, events: &Arc<dyn Events>) -> Result<()> {
    tasks
        .lock()
        .map_err(|_| anyhow!("file task state poisoned"))?
        .insert(task.id.clone(), task.clone());
    events.changed(task);
    Ok(())
}
async fn execute(
    request: Request,
    tasks: &Tasks,
    events: &Arc<dyn Events>,
    hooks: &Arc<dyn Hooks>,
) {
    let mut task = request.task;
    let result = async {
        hooks.authorize(task.kind, &request.ops).await?;
        let paths = request
            .ops
            .iter()
            .map(|op| op.src.clone())
            .collect::<Vec<_>>();
        let (bytes, items) = tokio::task::spawn_blocking(move || -> Result<(i64, i64)> {
            let mut bytes = 0_i64;
            let mut items = 0_i64;
            for path in paths {
                let (b, i) = plain_libs::filesystem::measure(Path::new(&path))?;
                bytes = bytes
                    .checked_add(b)
                    .ok_or_else(|| anyhow!("file size overflow"))?;
                items = items
                    .checked_add(i)
                    .ok_or_else(|| anyhow!("file count overflow"))?;
            }
            Ok((bytes, items))
        })
        .await??;
        task.total_bytes = bytes;
        task.total_items = items;
        task.status = FileTaskStatus::Running;
        task.updated_at = Utc::now();
        save(&task, tasks, events)?;
        for op in request.ops {
            hooks
                .authorize(task.kind, std::slice::from_ref(&op))
                .await?;
            let prepared = hooks.prepare(task.kind, &op).await?;
            hooks
                .authorize(task.kind, std::slice::from_ref(&op))
                .await?;
            let state = tasks.clone();
            let emitter = events.clone();
            let id = task.id.clone();
            let last_event = Mutex::new(std::time::Instant::now());
            let progress = move |bytes: i64, items: i64| -> std::io::Result<()> {
                let mut tasks = state
                    .lock()
                    .map_err(|_| std::io::Error::other("file task state poisoned"))?;
                let task = tasks
                    .get_mut(&id)
                    .ok_or_else(|| std::io::Error::other("file task disappeared"))?;
                task.done_bytes = task
                    .done_bytes
                    .checked_add(bytes)
                    .ok_or_else(|| std::io::Error::other("file size overflow"))?;
                task.done_items = task
                    .done_items
                    .checked_add(items)
                    .ok_or_else(|| std::io::Error::other("file count overflow"))?;
                task.updated_at = Utc::now();
                let mut last = last_event
                    .lock()
                    .map_err(|_| std::io::Error::other("file task event state poisoned"))?;
                if last.elapsed() >= std::time::Duration::from_millis(200) {
                    *last = std::time::Instant::now();
                    let snapshot = task.clone();
                    drop(tasks);
                    emitter.changed(&snapshot);
                }
                Ok(())
            };
            let destination = if request.rename {
                plain_libs::filesystem::rename_with_progress(
                    Path::new(&op.src),
                    Path::new(&op.dst),
                    Some(&progress),
                )
                .await?;
                Path::new(&op.dst).to_owned()
            } else {
                match task.kind {
                    FileTaskType::Copy => {
                        plain_libs::filesystem::copy_path_with_progress(
                            Path::new(&op.src),
                            Path::new(&op.dst),
                            op.overwrite,
                            Some(&progress),
                        )
                        .await?
                    }
                    FileTaskType::Move => {
                        plain_libs::filesystem::move_path_with_progress(
                            Path::new(&op.src),
                            Path::new(&op.dst),
                            op.overwrite,
                            Some(&progress),
                        )
                        .await?
                    }
                }
            };
            let completed = CompletedOp {
                src: op.src,
                dst: destination
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid destination encoding"))?
                    .into(),
            };
            {
                let mut tasks = tasks
                    .lock()
                    .map_err(|_| anyhow!("file task state poisoned"))?;
                tasks
                    .get_mut(&task.id)
                    .ok_or_else(|| anyhow!("file task disappeared"))?
                    .completed_ops
                    .push(completed.clone());
            }
            hooks
                .completed_with_snapshot(task.kind, &completed, &prepared)
                .await?;
        }
        Ok::<_, plain_libs::anyhow::Error>(())
    }
    .await;
    if let Ok(state) = tasks.lock() {
        if let Some(current) = state.get(&task.id) {
            task = current.clone();
        }
    }
    task.updated_at = Utc::now();
    match result {
        Ok(()) if task.done_bytes == task.total_bytes && task.done_items == task.total_items => {
            task.status = FileTaskStatus::Done;
            task.error.clear();
        }
        Ok(()) => {
            task.status = FileTaskStatus::Error;
            task.error = "source changed during file task".into();
        }
        Err(error) => {
            task.status = FileTaskStatus::Error;
            task.error = error.to_string();
        }
    }
    if let Err(error) = save(&task, tasks, events) {
        log::error!("file task state: {error}");
    }
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/files/tasks.rs"]
mod tests;
