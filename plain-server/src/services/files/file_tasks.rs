use super::platform::FilePlatform;
use anyhow::{Result, anyhow, bail};
use plain_rs::db::Db;
use plain_rs::events::WsEvent;
use plain_rs::files::tasks::CompletedOp;
use plain_rs::files::tasks::Events;
use plain_rs::files::tasks::FileTask;
use plain_rs::files::tasks::FileTaskOp;
use plain_rs::files::tasks::FileTaskStatus;
use plain_rs::files::tasks::FileTaskType;
use plain_rs::files::tasks::HookResult;
use plain_rs::files::tasks::Hooks;
use plain_rs::files::tasks::PrepareResult;
use plain_rs::files::tasks::Service;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
pub struct FileTasks {
    completion: Arc<tokio::sync::Notify>,
    deletion_lock: tokio::sync::Mutex<()>,
    events: broadcast::Sender<WsEvent>,
    service: Mutex<Option<Arc<Service>>>,
    hooks: Arc<NativeHooks>,
}
impl FileTasks {
    pub(crate) fn new(
        platform: Arc<FilePlatform>,
        events: broadcast::Sender<WsEvent>,
        audio: Arc<crate::services::audio::host::Audio>,
        index: Arc<crate::services::images::index::ImageIndex>,
        prefs: Arc<plain_rs::prefs::Prefs>,
        duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    ) -> Self {
        Self {
            completion: Arc::new(tokio::sync::Notify::new()),
            hooks: Arc::new(NativeHooks {
                platform,
                audio,
                index,
                prefs,
                duration_cache,
            }),
            deletion_lock: tokio::sync::Mutex::new(()),
            events,
            service: Mutex::new(None),
        }
    }
    pub(crate) async fn external_move(
        &self,
        source: &Path,
        destination: &Path,
        items: Vec<super::file_task_media::Identity>,
    ) -> Result<()> {
        let _lock = self.deletion_lock.lock().await;
        self.hooks
            .platform
            .authorize_metadata_paths(&[
                source.to_string_lossy().into_owned(),
                destination.to_string_lossy().into_owned(),
            ])
            .await?;
        self.hooks
            .platform
            .authorize(&destination.to_string_lossy(), false)
            .await?;
        match tokio::fs::symlink_metadata(source).await {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => bail!("external move source still exists"),
        }
        tokio::fs::symlink_metadata(destination).await?;
        let source = source
            .to_str()
            .ok_or_else(|| anyhow!("invalid external source encoding"))?;
        let destination = destination
            .to_str()
            .ok_or_else(|| anyhow!("invalid external destination encoding"))?;
        let paths = items
            .iter()
            .map(|item| {
                let relative = Path::new(&item.path).strip_prefix(source)?;
                let path = if relative.as_os_str().is_empty() {
                    PathBuf::from(destination)
                } else {
                    Path::new(destination).join(relative)
                };
                Ok((item.path.clone(), path.to_string_lossy().into_owned()))
            })
            .collect::<Result<Vec<_>>>()?;
        let snapshot = serde_json::json!({"root":source,"items":items});
        let op = CompletedOp {
            src: source.to_owned(),
            dst: destination.to_owned(),
        };
        self.hooks.prepare_move(&op, &snapshot).await?;
        for batch in paths.chunks(64) {
            let paths = batch
                .iter()
                .flat_map(|(source, destination)| [source.clone(), destination.clone()])
                .collect::<Vec<_>>();
            self.hooks.platform.scan(&paths).await?;
        }
        self.hooks.rebind_move(&op, &snapshot).await?;
        super::catalog_events::publish(
            &self.events,
            snapshot["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item["mediaType"].as_i64().map(|kind| kind as i32)),
        );
        Ok(())
    }

    pub(crate) async fn external_replace(
        &self,
        previous: super::file_task_media::Identity,
        current: &super::file_task_media::Identity,
    ) -> Result<()> {
        if previous.path != current.path || current.file_id.is_empty() {
            bail!("invalid replacement media receipt");
        }
        self.hooks.platform.authorize(&current.path, false).await?;
        if previous.media_type == current.media_type && previous.file_id == current.file_id {
            return Ok(());
        }
        let _lock = self.deletion_lock.lock().await;
        let op = CompletedOp {
            src: previous.path.clone(),
            dst: current.path.clone(),
        };
        let snapshot = serde_json::json!({"root":previous.path,"items":[previous.clone()]});
        self.hooks.prepare_move(&op, &snapshot).await?;
        if previous.media_type == current.media_type {
            self.hooks.rebind_move(&op, &snapshot).await?;
        } else {
            let index = self.hooks.index.clone();
            let prefs = self.hooks.prefs.clone();
            self.hooks
                .audio
                .run(move |db, engine| {
                    let before = plain_rs::media::audio::playback::snapshot(db)?;
                    let item = plain_rs::files::deletes::Item {
                        media_type: previous.media_type,
                        id: previous.file_id,
                        path: previous.path,
                    };
                    index
                        .update_cache(|db| plain_rs::files::deletes::retire_identity(db, &item))?;
                    if item.media_type == 1 && before.path == item.path {
                        plain_rs::media::audio::commands::clear_engine(db, &prefs, engine)?;
                    }
                    Ok(())
                })
                .await
                .map_err(|error| anyhow!(error.message))?;
        }
        Ok(())
    }

    pub(crate) async fn external_remove(
        &self,
        root: &Path,
        items: Vec<super::file_task_media::Identity>,
    ) -> Result<()> {
        let _lock = self.deletion_lock.lock().await;
        let root = root
            .to_str()
            .ok_or_else(|| anyhow!("invalid external path encoding"))?;
        let mut paths = items
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>();
        paths.push(root.to_owned());
        paths.sort();
        paths.dedup();
        let snapshot = serde_json::json!({"root":root,"items":items});
        let outcome = plain_rs::files::filesystem::deletion::Outcome {
            removed: true,
            paths,
            failures: Vec::new(),
        };
        self.finish_deletion(root, root, &snapshot, &outcome)
            .await?;
        super::catalog_events::publish(
            &self.events,
            snapshot["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item["mediaType"].as_i64().map(|kind| kind as i32)),
        );
        Ok(())
    }

    fn service(&self) -> Result<Arc<Service>> {
        let mut service = self
            .service
            .lock()
            .map_err(|_| anyhow!("file task service poisoned"))?;
        if let Some(service) = service.as_ref() {
            return Ok(service.clone());
        }
        tokio::runtime::Handle::try_current()?;
        let next = Arc::new(Service::with_hooks(
            Arc::new(Changes {
                events: self.events.clone(),
                completion: self.completion.clone(),
            }),
            self.hooks.clone(),
        ));
        *service = Some(next.clone());
        Ok(next)
    }
    pub async fn create(
        &self,
        client_id: &str,
        kind: FileTaskType,
        title: &str,
        ops: Vec<FileTaskOp>,
    ) -> Result<FileTask> {
        if client_id.is_empty() {
            bail!("unauthorized");
        }
        if ops.is_empty()
            || ops.iter().any(|op| {
                !PathBuf::from(&op.src).is_absolute() || !PathBuf::from(&op.dst).is_absolute()
            })
        {
            bail!("absolute file paths required");
        }
        self.hooks.authorize(kind, &ops).await?;
        preflight_sources(&ops).await?;
        self.service()?.create(client_id, kind, title, ops)
    }
    pub async fn execute(
        &self,
        client_id: &str,
        kind: FileTaskType,
        ops: Vec<FileTaskOp>,
    ) -> Result<FileTask> {
        self.create_and_wait(client_id, kind, &format!("{kind:?}").to_uppercase(), ops)
            .await
    }
    pub async fn create_and_wait(
        &self,
        client_id: &str,
        kind: FileTaskType,
        title: &str,
        ops: Vec<FileTaskOp>,
    ) -> Result<FileTask> {
        let task = self.create(client_id, kind, title, ops).await?;
        let service = self.service()?;
        wait_for_completion(&self.completion, || {
            let current = service
                .get(&task.id)?
                .ok_or_else(|| anyhow!("file task unavailable"))?;
            Ok(
                matches!(current.status, FileTaskStatus::Done | FileTaskStatus::Error)
                    .then_some(current),
            )
        })
        .await
    }
    pub async fn copy_to_directory(
        &self,
        client_id: &str,
        src: String,
        directory: String,
        name: String,
    ) -> Result<Option<String>> {
        let directory = named_directory(&directory, &name)?;
        let src = plain_rs::utils::file_uri::local_path(&src).map_err(anyhow::Error::msg)?;
        let destination = directory.join(name);
        let destination = tokio::task::spawn_blocking(move || {
            plain_rs::utils::unique_path::unique_sibling(&destination)
        })
        .await??;
        let task = self
            .execute(
                client_id,
                FileTaskType::Copy,
                vec![FileTaskOp {
                    src,
                    dst: destination.to_string_lossy().into_owned(),
                    overwrite: false,
                }],
            )
            .await?;
        Ok(if task.status == FileTaskStatus::Done {
            task.completed_ops
                .first()
                .map(|operation| operation.dst.clone())
        } else {
            None
        })
    }
    pub async fn list(&self, client_id: String) -> Result<Vec<FileTask>> {
        let service = self.service()?;
        tokio::task::spawn_blocking(move || service.list(&client_id)).await?
    }
}
async fn preflight_sources(ops: &[FileTaskOp]) -> Result<()> {
    let sources = ops.iter().map(|op| op.src.clone()).collect::<Vec<_>>();
    tokio::task::spawn_blocking(move || -> Result<()> {
        for source in sources {
            std::fs::metadata(source)?;
        }
        Ok(())
    })
    .await?
}

pub(crate) fn named_directory(directory: &str, name: &str) -> Result<PathBuf> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\\', '\0']) {
        bail!("invalid file name");
    }
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() {
        bail!("absolute destination directory required");
    }
    Ok(directory)
}

async fn wait_for_completion<T>(
    completion: &tokio::sync::Notify,
    mut read: impl FnMut() -> Result<Option<T>>,
) -> Result<T> {
    loop {
        let notification = completion.notified();
        tokio::pin!(notification);
        notification.as_mut().enable();
        if let Some(value) = read()? {
            return Ok(value);
        }
        notification.await;
    }
}
struct Changes {
    events: broadcast::Sender<WsEvent>,
    completion: Arc<tokio::sync::Notify>,
}
impl Events for Changes {
    fn changed(&self, task: &FileTask) {
        if matches!(task.status, FileTaskStatus::Done | FileTaskStatus::Error) {
            let _ = self
                .events
                .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
            self.completion.notify_waiters();
        }
    }
}
struct NativeHooks {
    duration_cache: Arc<plain_rs::media::duration_cache::DurationCache>,
    platform: Arc<FilePlatform>,
    audio: Arc<crate::services::audio::host::Audio>,
    index: Arc<crate::services::images::index::ImageIndex>,
    prefs: Arc<plain_rs::prefs::Prefs>,
}
impl Hooks for NativeHooks {
    fn prepare<'a>(&'a self, kind: FileTaskType, op: &'a FileTaskOp) -> PrepareResult<'a> {
        Box::pin(async move {
            if kind == FileTaskType::Move {
                crate::services::files::file_task_media::prepare(&self.platform, &op.src).await
            } else {
                Ok(serde_json::Value::Null)
            }
        })
    }
    fn completed_with_snapshot<'a>(
        &'a self,
        kind: FileTaskType,
        op: &'a CompletedOp,
        snapshot: &'a serde_json::Value,
    ) -> HookResult<'a> {
        self.apply_completed(kind, op, snapshot)
    }
    fn authorize<'a>(&'a self, kind: FileTaskType, ops: &'a [FileTaskOp]) -> HookResult<'a> {
        Box::pin(async move { self.platform.authorize_task(kind, ops).await })
    }
    fn completed<'a>(&'a self, kind: FileTaskType, op: &'a CompletedOp) -> HookResult<'a> {
        self.scan_completed(kind, op)
    }
}
impl NativeHooks {
    async fn prepare_move(&self, op: &CompletedOp, snapshot: &serde_json::Value) -> Result<()> {
        let source = crate::services::files::file_task_media::source_root(snapshot)?;
        let destination =
            crate::services::files::file_task_media::normalized(std::path::Path::new(&op.dst))
                .await?;
        if !self.platform.host.uses_system_media_catalog() {
            self.platform.db.clone().change_file_paths(
                &source,
                &destination,
                plain_rs::db::files::PathChange::Move,
            )?;
        }
        self.duration_cache
            .invalidate_paths(&[], &[op.src.clone(), op.dst.clone(), source, destination])?;
        Ok(())
    }
    async fn rebind_move(&self, op: &CompletedOp, snapshot: &serde_json::Value) -> Result<()> {
        let change =
            crate::services::files::file_task_media::resolve(&self.platform, &op.dst, snapshot)
                .await?;
        let removed_sources = change
            .bindings
            .iter()
            .map(|binding| binding.source_path.clone())
            .collect::<Vec<_>>();
        let raw_source = op.src.clone();
        let raw_destination = op.dst.clone();
        let prefs = self.prefs.clone();
        let index = self.index.clone();
        self.audio
            .run(move |db, engine| {
                use plain_rs::files::moves as media_moves;
                use plain_rs::media::audio::playback as audio_playback;
                let before = audio_playback::snapshot(db)?;
                let source_prefix = format!("{}/", change.source_root.trim_end_matches('/'));
                let moved_audio = !before.path.is_empty()
                    && (before.path == change.source_root
                        || before.path.starts_with(&source_prefix)
                        || before.path == raw_source
                        || before
                            .path
                            .starts_with(&format!("{}/", raw_source.trim_end_matches('/'))));
                let rebind = |db: &Db| {
                    media_moves::rebind_with_aliases(
                        db,
                        &change.bindings,
                        &change.source_root,
                        &change.destination_root,
                        &[(raw_source, raw_destination)],
                    )
                };
                index.update_cache(rebind)?;
                if moved_audio {
                    use plain_rs::media::audio::commands;
                    commands::clear_engine(db, &prefs, engine)?;
                }
                Ok(())
            })
            .await
            .map_err(|e| anyhow!(e.message))?;
        self.platform.forget_removed(removed_sources).await?;
        Ok(())
    }
    fn apply_completed<'a>(
        &'a self,
        kind: FileTaskType,
        op: &'a CompletedOp,
        snapshot: &'a serde_json::Value,
    ) -> HookResult<'a> {
        Box::pin(async move {
            if kind == FileTaskType::Move {
                self.prepare_move(op, snapshot).await?;
            }
            self.completed(kind, op).await?;
            if kind == FileTaskType::Move {
                self.rebind_move(op, snapshot).await?;
            }
            Ok(())
        })
    }
    fn scan_completed<'a>(&'a self, kind: FileTaskType, op: &'a CompletedOp) -> HookResult<'a> {
        Box::pin(async move {
            let root = PathBuf::from(&op.dst);
            let mut walker = if self.platform.host.uses_system_media_catalog() {
                crate::services::files::file_task_walk::FileWalker::new(&root)
            } else {
                crate::services::files::file_task_walk::FileWalker::entries(&root)
            };
            let mut batch = Vec::new();
            while let Some(path) = walker.next().await? {
                let info = tokio::fs::symlink_metadata(&path).await?;
                if info.is_file()
                    || (info.is_dir() && !self.platform.host.uses_system_media_catalog())
                {
                    batch.push(
                        path.to_str()
                            .ok_or_else(|| anyhow!("invalid path encoding"))?
                            .to_owned(),
                    );
                    if kind == FileTaskType::Move {
                        let relative = path.strip_prefix(&root)?;
                        let source = if relative.as_os_str().is_empty() {
                            PathBuf::from(&op.src)
                        } else {
                            PathBuf::from(&op.src).join(relative)
                        };
                        batch.push(
                            source
                                .to_str()
                                .ok_or_else(|| anyhow!("invalid path encoding"))?
                                .to_owned(),
                        );
                    }
                    if batch.len() >= 128 {
                        self.platform.scan(&batch).await?;
                        batch.clear();
                    }
                }
            }
            if !batch.is_empty() {
                self.platform.scan(&batch).await?;
            }
            Ok(())
        })
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/services/files/file_tasks.rs"]
mod tests;

#[cfg(feature = "core")]
pub mod mutations;

mod trash;
