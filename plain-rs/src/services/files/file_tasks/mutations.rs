use plain_rs::files::filesystem::deletion::Outcome;
use plain_rs::files::filesystem::deletion::Plan;
use plain_rs::files::tasks::FileTaskOp;
use plain_rs::files::tasks::FileTaskStatus;
use plain_rs::files::tasks::FileTaskType;
use plain_rs::files::tasks::Hooks;
use plain_rs::services::files::file_tasks::*;
impl FileTasks {
    pub async fn rename(
        &self,
        client_id: &str,
        path: String,
        name: String,
    ) -> Result<Option<String>> {
        if name.is_empty()
            || matches!(name.as_str(), "." | "..")
            || name.contains('/')
            || name.contains('\\')
        {
            return Ok(None);
        }
        let source = PathBuf::from(&path);
        if !source.is_absolute() || source.file_name().is_none() {
            bail!("absolute file path required");
        }
        let destination = source
            .parent()
            .ok_or_else(|| anyhow!("missing file parent"))?
            .join(&name);
        let destination = destination
            .to_str()
            .ok_or_else(|| anyhow!("invalid filename encoding"))?
            .to_owned();
        let op = FileTaskOp {
            src: path.clone(),
            dst: destination.clone(),
            overwrite: false,
        };
        self.hooks
            .authorize(FileTaskType::Move, std::slice::from_ref(&op))
            .await?;
        if tokio::fs::symlink_metadata(&destination).await.is_ok() {
            return Ok(None);
        }
        if client_id.is_empty() {
            bail!("unauthorized");
        }
        let service = self.service()?;
        let task = service.rename(client_id, path, destination.clone())?;
        let current = wait_for_completion(&self.completion, || {
            let current = service
                .get(&task.id)?
                .ok_or_else(|| anyhow!("rename task unavailable"))?;
            Ok(
                matches!(current.status, FileTaskStatus::Done | FileTaskStatus::Error)
                    .then_some(current),
            )
        })
        .await?;
        if current.status == FileTaskStatus::Error {
            if current.completed_ops.is_empty() {
                return Ok(None);
            }
            bail!(
                "rename postprocessing failed for task {}: {}",
                current.id,
                current.error
            );
        }
        Ok(Some(destination))
    }
    pub async fn delete(&self, path: String) -> Result<Outcome> {
        let _guard = self.deletion_lock.lock().await;
        plain_rs::files::filesystem::deletion::validate(std::path::Path::new(&path))?;
        self.hooks.platform.authorize(&path, true).await?;
        self.delete_prepared(path).await
    }
    pub async fn delete_many(&self, paths: Vec<String>) -> Result<i32> {
        let _guard = self.deletion_lock.lock().await;
        let mut seen = std::collections::HashSet::new();
        let paths = paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect::<Vec<_>>();
        for path in &paths {
            plain_rs::files::filesystem::deletion::validate(std::path::Path::new(path))?;
        }
        if paths.is_empty() {
            return Ok(0);
        }
        self.hooks.platform.authorize_paths(&paths, true).await?;
        let mut deleted = 0i32;
        for path in paths {
            let outcome = self.delete_prepared(path).await?;
            if !outcome.failures.is_empty() {
                bail!(
                    "File deletion failed: {}",
                    outcome
                        .failures
                        .iter()
                        .map(|failure| format!("{}: {}", failure.path, failure.error))
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }
            if outcome.removed {
                deleted = deleted
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("Deletion count overflow"))?;
            }
        }
        Ok(deleted)
    }
    async fn delete_prepared(&self, path: String) -> Result<Outcome> {
        match tokio::fs::symlink_metadata(&path).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Outcome {
                    removed: false,
                    paths: Vec::new(),
                    failures: Vec::new(),
                });
            }
            Err(e) => return Err(e.into()),
            Ok(_) => {}
        }
        let root =
            plain_rs::services::files::file_task_media::normalized(std::path::Path::new(&path))
                .await?;
        let checked = root.clone();
        let plan =
            tokio::task::spawn_blocking(move || Plan::inspect(std::path::Path::new(&checked)))
                .await??;
        let snapshot = plain_rs::services::files::file_task_media::deletion_snapshot(
            &self.hooks.platform,
            root.clone(),
            &plan.files,
        )
        .await?;
        self.hooks.platform.authorize(&root, true).await?;
        let outcome = tokio::task::spawn_blocking(move || plan.execute()).await?;
        self.finish_deletion(&path, &root, &snapshot, &outcome)
            .await?;
        Ok(outcome)
    }
    pub async fn finish_deletion(
        &self,
        path: &str,
        root: &str,
        snapshot: &serde_json::Value,
        outcome: &Outcome,
    ) -> Result<()> {
        let root = root.to_owned();
        let path = path.to_owned();
        let snapshot = snapshot.clone();
        for path in &outcome.paths {
            require_absent(path)?;
        }
        if !outcome.paths.is_empty() {
            let items = plain_rs::services::files::file_task_media::deleted_items(
                &snapshot,
                &outcome.paths,
            )?;
            let mut removed_paths = outcome.paths.clone();
            for removed in &outcome.paths {
                let relative = std::path::Path::new(removed).strip_prefix(&root)?;
                let alias = if relative.as_os_str().is_empty() {
                    path.clone()
                } else {
                    std::path::Path::new(&path)
                        .join(relative)
                        .to_string_lossy()
                        .into_owned()
                };
                if !removed_paths.contains(&alias) {
                    removed_paths.push(alias);
                }
            }
            let roots = if outcome.removed {
                vec![root.clone(), path.clone()]
            } else {
                Vec::new()
            };
            self.hooks
                .duration_cache
                .invalidate_paths(&removed_paths, &roots)?;
            let prefs = self.hooks.prefs.clone();
            let index = self.hooks.index.clone();
            self.hooks
                .audio
                .run(move |db, engine| {
                    use plain_rs::files::deletes as media_deletes;
                    use plain_rs::media::audio::commands as audio_commands;
                    use plain_rs::media::audio::playback as audio_playback;
                    let before = audio_playback::snapshot(db)?;
                    let stop = removed_paths.contains(&before.path)
                        || roots.iter().any(|root| {
                            before.path == *root
                                || before
                                    .path
                                    .starts_with(&format!("{}/", root.trim_end_matches('/')))
                        });
                    let cleanup = |db: &Db| {
                        for path in &removed_paths {
                            require_absent(path)
                                .map_err(|e| plain_rs::error::DomainError::Other(e.to_string()))?;
                        }
                        media_deletes::cleanup(db, &items, &removed_paths, &roots)
                    };
                    index.update_cache(cleanup)?;
                    if stop && !before.path.is_empty() {
                        audio_commands::command(
                            db,
                            &prefs,
                            engine,
                            audio_commands::Action::Clear,
                            0,
                            1.0,
                        )?;
                    }
                    Ok(())
                })
                .await
                .map_err(|e| anyhow!(e.to_string()))?;
            let _ = self
                .events
                .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
            for paths in outcome.paths.chunks(128) {
                self.hooks.platform.scan(paths).await?;
            }
        }
        Ok(())
    }
}

fn require_absent(path: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(_) => bail!("deleted path reappeared: {path}"),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/files/file_tasks/mutations.rs"]
mod tests;
