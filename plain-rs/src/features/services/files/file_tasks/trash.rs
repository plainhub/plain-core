use crate::features::files::trash::Trash;
use crate::features::services::files::file_tasks::*;
use plain_libs::rusqlite::OptionalExtension;

impl FileTasks {
    pub async fn trash_paths(&self, paths: Vec<String>) -> Result<usize> {
        let _guard = self.deletion_lock.lock().await;
        use crate::features::enums::DataType;
        use crate::features::media::actions::Action;
        let mut roots = Vec::with_capacity(paths.len());
        let mut files = Vec::new();
        let provider = self.has_provider_trash().await?;
        for path in paths {
            plain_libs::filesystem::deletion::validate(Path::new(&path))?;
            self.hooks.platform.authorize(&path, true).await?;
            let root =
                crate::features::services::files::file_task_media::normalized(Path::new(&path))
                    .await?;
            if crate::features::files::trash::is_trashed_path(&root) {
                bail!("File is already in trash");
            }
            let metadata = tokio::fs::symlink_metadata(&root).await?;
            if provider && metadata.is_file() {
                files.push(root.clone());
            }
            roots.push(root);
        }
        let mut identities = std::collections::HashMap::new();
        for batch in files.chunks(128) {
            for row in crate::features::services::files::file_task_media::lookup(
                &self.hooks.platform,
                batch,
            )
            .await?
            {
                identities.insert(row.path.clone(), row);
            }
        }
        let trash = Trash::new(self.hooks.platform.db.clone());
        let mut count = 0;
        for root in roots {
            if let Some(identity) = identities.get(&root) {
                let kind = DataType::from_kind(identity.media_type)
                    .ok_or_else(|| anyhow!("Unknown media type"))?;
                self.media_actions
                    .apply_file(kind, Action::Trash, identity.file_id.clone(), root)
                    .await?;
                count += 1;
                continue;
            }
            trash.trash_paths(vec![root.clone()]).await?;
            self.hooks
                .duration_cache
                .invalidate_paths(&[], &[root.clone()])?;
            let _ = self
                .events
                .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
            let prefs = self.hooks.prefs.clone();
            self.hooks
                .audio
                .run(move |db, engine| {
                    let trashed = if let Some(id) = db.audio_current_file_id() {
                        db.with_conn(|conn| {
                            conn.query_row(
                                "SELECT trashed FROM file_catalog WHERE id=?1",
                                [id],
                                |row| row.get::<_, bool>(0),
                            )
                            .optional()
                        })?
                        .unwrap_or(false)
                    } else {
                        false
                    };
                    if trashed {
                        crate::features::media::audio::commands::clear_engine(db, &prefs, engine)?;
                    }
                    Ok(())
                })
                .await
                .map_err(|error| anyhow!(error.to_string()))?;
            count += 1;
        }
        Ok(count)
    }
    pub async fn restore_paths(&self, references: Vec<String>) -> Result<usize> {
        let _guard = self.deletion_lock.lock().await;
        let trash = Trash::new(self.hooks.platform.db.clone());
        let mut count = 0;
        let mut current = self.selected_provider_trash(&references).await?;
        for reference in references {
            if let Some(restored) = self
                .apply_provider_path(
                    &reference,
                    crate::features::media::actions::Action::Restore,
                    &mut current,
                )
                .await?
            {
                if !restored {
                    bail!("Trash item not found");
                }
                count += 1;
                continue;
            }
            let item = trash
                .resolve_item(&reference)?
                .ok_or_else(|| anyhow!("trash item not found"))?;
            self.hooks
                .platform
                .authorize(&item.original_path, true)
                .await?;
            let physical = Trash::physical_path(&item).to_string_lossy().into_owned();
            self.hooks.platform.authorize(&physical, false).await?;
            for restored in trash.restore_paths(vec![item.id]).await? {
                self.hooks
                    .scan_completed(
                        FileTaskType::Copy,
                        &CompletedOp {
                            src: physical.clone(),
                            dst: restored,
                        },
                    )
                    .await?;
                count += 1;
            }
        }
        let _ = self
            .events
            .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
        Ok(count)
    }
    pub async fn delete_trashed(&self, references: Vec<String>) -> Result<usize> {
        let _guard = self.deletion_lock.lock().await;
        let mut current = self.selected_provider_trash(&references).await?;
        let trash = Trash::new(self.hooks.platform.db.clone());
        let mut count = 0;
        for reference in references {
            if let Some(deleted) = self
                .apply_provider_path(
                    &reference,
                    crate::features::media::actions::Action::Delete,
                    &mut current,
                )
                .await?
            {
                count += usize::from(deleted);
                continue;
            }
            let Some(item) = trash.resolve_item(&reference)? else {
                continue;
            };
            self.hooks
                .platform
                .authorize(&item.path.to_string_lossy(), true)
                .await?;
            trash.delete_trash_by_path(&item.id).await?;
            let _ = self
                .events
                .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
            count += 1;
        }
        Ok(count)
    }
    pub async fn trashed(
        &self,
        offset: usize,
        limit: usize,
        query: String,
        show_hidden: bool,
    ) -> Result<Vec<plain_libs::filesystem::record::FileRecord>> {
        let db = self.hooks.platform.db.clone();
        let provider = self.provider_trash().await?.into_iter().map(|(row, _)| row).collect();
        let rows = tokio::task::spawn_blocking(move || {
            let trash = Trash::new(db);
            trash.list_with(provider, offset, limit, &query)
        })
        .await??;
        let platform = &self.hooks.platform;
        let paths = rows.iter().map(|row| row.path.to_string_lossy().into_owned()).collect::<Vec<_>>();
        platform.authorize_metadata_paths(&paths).await?;
        let excluded = platform.protected_files();
        tokio::task::spawn_blocking(move || rows.into_iter().map(|row| {
            Ok(excluded.file_row(&row.path, row.name().to_owned(), show_hidden)?)
        }).collect()).await?
    }
    pub async fn trash_count(&self, query: String) -> Result<usize> {
        let db = self.hooks.platform.db.clone();
        let needle = query.trim().to_lowercase();
        let provider = self
            .provider_trash()
            .await?
            .into_iter()
            .filter(|(row, _)| row.original_path.to_lowercase().contains(&needle))
            .count();
        Ok(provider
            + tokio::task::spawn_blocking(move || Trash::new(db).trash_count(&query)).await??)
    }
}
