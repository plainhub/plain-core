use crate::assembly::server::ServerState;
use plain_rs::db::{MediaFileMetadata, MediaFileRow};
use plain_rs::enums::DataType;

#[cfg(feature = "media")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CompletedFilesRequest {
    paths: Vec<String>,
}

#[derive(Clone)]
pub struct NativeMediaCatalog {
    state: ServerState,
    #[cfg(test)]
    read_gate: Option<std::sync::Arc<(tokio::sync::Notify, tokio::sync::Semaphore)>>,
}

impl NativeMediaCatalog {
    pub(crate) fn new(state: ServerState) -> Self {
        Self {
            state,
            #[cfg(test)]
            read_gate: None,
        }
    }

    pub fn source_directories(&self) -> Result<Vec<String>, String> {
        self.state.native_client_id()?;
        plain_rs::prefs::media::source_directories(&self.state.prefs)
            .map_err(|error| error.to_string())
    }

    pub fn set_source_directories(&self, directories: &[String]) -> Result<(), String> {
        self.state.native_client_id()?;
        plain_rs::prefs::media::set_source_directories(&self.state.prefs, directories)
            .map_err(|error| error.to_string())
    }

    #[cfg(feature = "media")]
    pub async fn scan_file(
        &self,
        data_type: DataType,
        reference: &str,
    ) -> Result<MediaFileRow, String> {
        let metadata = self.read_file(data_type, reference).await?;
        self.record_file_reads(&[metadata])?
            .into_iter()
            .next()
            .ok_or_else(|| "Missing recorded media file".into())
    }

    #[cfg(feature = "media")]
    async fn read_file(
        &self,
        data_type: DataType,
        reference: &str,
    ) -> Result<plain_rs::db::MediaFileMetadataRead, String> {
        #[cfg(test)]
        if let Some(gate) = &self.read_gate {
            gate.0.notify_one();
            gate.1
                .acquire()
                .await
                .map_err(|error| error.to_string())?
                .forget();
        }
        let path =
            crate::services::files::file_access::resolve_native(&self.state, reference).await?;
        let metadata = tokio::task::spawn_blocking(move || {
            plain_media::metadata::file_facts::read_file_metadata_with_availability(
                &path, data_type,
            )
            .map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(metadata)
    }

    #[cfg(feature = "media")]
    pub async fn scan_files(
        &self,
        data_type: DataType,
        references: Vec<String>,
    ) -> Result<crate::services::files::media_scans::Snapshot, String> {
        self.state.native_client_id()?;
        data_type
            .media_type_str()
            .ok_or("Unsupported media catalog type")?;
        let session = self.state.media_scans.begin(references.len())?;
        let mut cancel = session.cancel.clone();
        let mut stop = self.state.stop.clone();
        let result = async {
            for reference in references {
                session.wait_running().await?;
                let metadata = tokio::select! {
                    biased;
                    _ = crate::services::files::media_scans::wait_stop(&mut cancel) => return Err("Media scan cancelled".into()),
                    _ = crate::services::files::media_scans::wait_stop(&mut stop) => {
                        self.state.media_scans.cancel()?;
                        return Err("Rust core stopped".into());
                    },
                    metadata = self.read_file(data_type, &reference) => metadata?,
                };
                session.record_when_running(1,|| self.persist_file_reads(&[metadata])).await?;
            }
            Ok(())
        }.await;
        session.finish(result)
    }

    #[cfg(feature = "media")]
    pub fn scan_completed_files(&self, params: serde_json::Value) -> Result<bool, String> {
        self.state.native_client_id()?;
        let request: CompletedFilesRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        crate::services::files::native_scan::completed(
            &self.state.db,
            &self.state.media_scans,
            &self.state.directory,
            request.paths,
        )?;
        Ok(true)
    }

    #[cfg(feature = "media")]
    pub fn configure_scan_directories(
        &self,
        config_file: Option<&std::path::Path>,
    ) -> Result<(), String> {
        self.state.native_client_id()?;
        self.state
            .media_scans
            .configure(crate::services::files::media_scan_policy::Policy::load(
                self.state.cache.root(),
                config_file,
            )?)
    }

    #[cfg(feature = "media")]
    pub async fn scan_directory(
        &self,
        reference: &str,
    ) -> Result<crate::services::files::media_scans::Snapshot, String> {
        self.scan_directories(vec![reference.to_owned()]).await
    }

    #[cfg(feature = "media")]
    pub fn watch_source_directories(&self) -> Result<bool, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .refresh_volume_availability()
            .map_err(|error| error.to_string())?;
        let roots = self
            .source_directories()?
            .into_iter()
            .map(std::path::PathBuf::from)
            .filter(|root| root.is_dir())
            .collect::<Vec<_>>();
        if roots.is_empty() {
            self.state
                .media_watch
                .lock()
                .map_err(|_| "Media watcher unavailable")?
                .take();
            return Ok(false);
        }
        let catalog_state = self.state.clone();
        let handle = crate::services::files::media_watch::start(&roots, move |event| {
            let catalog = Self::new(catalog_state.clone());
            async move { catalog.apply_watch_event(event).await }
        })?;
        let mut slot = self
            .state
            .media_watch
            .lock()
            .map_err(|_| "media watch unavailable")?;
        *slot = Some(handle);
        Ok(true)
    }

    #[cfg(feature = "media")]
    async fn apply_watch_event(&self, event: notify::Event) -> Result<(), String> {
        self.state
            .db
            .refresh_volume_availability()
            .map_err(|error| error.to_string())?;
        use crate::services::files::media_watch::{Effect, effect_for};
        use notify::event::{EventKind, ModifyKind, RenameMode};
        if matches!(event.kind, EventKind::Other) {
            self.scan_directories(self.source_directories()?).await?;
            return Ok(());
        }
        if matches!(
            event.kind,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        ) && event.paths.len() == 2
        {
            let source = &event.paths[0];
            let destination = &event.paths[1];
            if !source.try_exists().map_err(|error| error.to_string())?
                && destination
                    .try_exists()
                    .map_err(|error| error.to_string())?
                && self.state.file_platform.watchable(destination)
            {
                let items = self.path_identities(source)?;
                self.state
                    .files
                    .external_move(source, destination, items)
                    .await
                    .map_err(|error| error.to_string())?;
                if destination.is_dir() {
                    self.scan_directory(&destination.to_string_lossy()).await?;
                } else {
                    self.state
                        .file_platform
                        .scan(&[destination.to_string_lossy().into_owned()])
                        .await
                        .map_err(|error| error.to_string())?;
                }
                return Ok(());
            }
        }
        let policy = self.state.media_scans.policy()?;
        for path in event.paths {
            let excluded = plain_rs::media::catalog_paths::is_media_excluded_at(
                &path.to_string_lossy(),
                &self.state.directory,
                &policy.cache_directory,
                &policy.excluded_roots,
            );
            match effect_for(&path, &event.kind, excluded) {
                Effect::Index => {
                    self.state
                        .file_platform
                        .scan(&[path.to_string_lossy().into_owned()])
                        .await
                        .map_err(|error| error.to_string())?;
                }
                Effect::Forget => {
                    if let Some(parent) = path.parent().filter(|parent| parent.is_dir()) {
                        self.scan_directory(&parent.to_string_lossy()).await?;
                    }
                }
                Effect::RescanSubtree => {
                    let root = if path.is_dir() {
                        path.as_path()
                    } else {
                        path.parent().ok_or("Media directory parent required")?
                    };
                    self.scan_directory(&root.to_string_lossy()).await?;
                }
                Effect::Ignore => {}
            }
        }
        Ok(())
    }

    #[cfg(feature = "media")]
    fn path_identities(
        &self,
        root: &std::path::Path,
    ) -> Result<Vec<crate::services::files::file_task_media::Identity>, String> {
        let root = root.to_str().ok_or("Invalid media path encoding")?;
        let prefix = format!("{}/", root.trim_end_matches('/'));
        self.state.db.with_conn(|connection| {
            connection.prepare("SELECT data_type,id,path FROM file_catalog WHERE missing=0 AND trashed=0 AND (path=?1 OR substr(path,1,length(?2))=?2)")?
                .query_map(rusqlite::params![root,prefix], |row| Ok(crate::services::files::file_task_media::Identity { media_type:row.get(0)?, file_id:row.get(1)?, path:row.get(2)? }))?
                .collect::<rusqlite::Result<Vec<_>>>()
        }).map_err(|error| error.to_string())
    }

    #[cfg(feature = "media")]
    pub async fn scan_source_directories(
        &self,
        root: &str,
    ) -> Result<crate::services::files::media_scans::Snapshot, String> {
        self.scan_directories(self.source_references(root)?).await
    }

    #[cfg(feature = "media")]
    fn source_references(&self, root: &str) -> Result<Vec<String>, String> {
        let sources = self.source_directories()?;
        plain_rs::media::catalog_paths::selected_roots(root.to_owned(), &sources)
            .into_iter()
            .map(|path| {
                path.into_os_string()
                    .into_string()
                    .map_err(|_| "Invalid media directory path encoding".to_string())
            })
            .collect::<Result<Vec<_>, _>>()
    }

    #[cfg(feature = "media")]
    pub fn start_source_scan(
        &self,
        root: &str,
    ) -> Result<crate::services::files::media_scans::Snapshot, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|error| error.to_string())?;
        let references = self.source_references(root)?;
        let policy = self.state.media_scans.policy()?;
        let session = self.state.media_scans.begin_discovery()?;
        let catalog = self.clone();
        self.state
            .media_scans
            .spawn(&runtime, session, move |session| {
                Box::pin(async move {
                    if let Err(error) = catalog
                        .scan_directory_session(references, policy, session)
                        .await
                    {
                        log::error!("Media scan task failed: {error}");
                    }
                })
            })
    }

    #[cfg(feature = "media")]
    pub async fn scan_directories(
        &self,
        references: Vec<String>,
    ) -> Result<crate::services::files::media_scans::Snapshot, String> {
        self.state.native_client_id()?;
        let policy = self.state.media_scans.policy()?;
        let session = self.state.media_scans.begin_discovery()?;
        self.scan_directory_session(references, policy, session)
            .await
    }

    #[cfg(feature = "media")]
    async fn scan_directory_session(
        &self,
        references: Vec<String>,
        policy: std::sync::Arc<crate::services::files::media_scan_policy::Policy>,
        session: crate::services::files::media_scans::Session,
    ) -> Result<crate::services::files::media_scans::Snapshot, String> {
        let mut cancel = session.cancel.clone();
        let mut stop = self.state.stop.clone();
        let worker_cancel = cancel.clone();
        let worker_stop = stop.clone();
        let result = async {
            let mut total_files = 0usize;
            let mut completed_roots = Vec::new();
            for reference in references {
                session.wait_running().await?;
                let root =
                    crate::services::files::file_access::resolve_native(&self.state, &reference)
                        .await?;
                let canonical_root = tokio::fs::canonicalize(&root)
                    .await
                    .map_err(|error| error.to_string())?;
                if !tokio::fs::symlink_metadata(&root)
                    .await
                    .map_err(|error| error.to_string())?
                    .file_type()
                    .is_dir()
                {
                    return Err("Real media scan directory required".into());
                }
                let (send, mut receive) =
                    tokio::sync::mpsc::channel::<(DataType, std::path::PathBuf)>(32);
                let directory = tokio::fs::canonicalize(&self.state.directory)
                    .await
                    .map_err(|error| error.to_string())?;
                let policy = policy.clone();
                let cancelled = worker_cancel.clone();
                let stopped = worker_stop.clone();
                let producer_root = canonical_root.clone();
                let producer = tokio::task::spawn_blocking(move || -> Result<usize, String> {
                    let mut discovered = 0;
                    plain_media::file_walk::walk_entries(
                        &producer_root,
                        |path| {
                            path.to_str().is_some_and(|path| {
                                plain_rs::media::catalog_paths::is_file_excluded_at(
                                    path,
                                    &directory,
                                    &policy.cache_directory,
                                    &policy.excluded_roots,
                                )
                            })
                        },
                        || {
                            *cancelled.borrow()
                                || cancelled.has_changed().is_err()
                                || *stopped.borrow()
                                || stopped.has_changed().is_err()
                        },
                        |path| {
                            let name = path.to_str().ok_or_else(|| {
                                anyhow::anyhow!("Invalid media file path encoding")
                            })?;
                            let kind = plain_rs::media::catalog_paths::infer_type(name);
                            let data_type = if path.is_file()
                                && !plain_rs::media::catalog_paths::is_media_excluded_at(
                                    name,
                                    &directory,
                                    &policy.cache_directory,
                                    &policy.excluded_roots,
                                ) {
                                [
                                    DataType::Audio,
                                    DataType::Video,
                                    DataType::Image,
                                    DataType::Doc,
                                ]
                                .into_iter()
                                .find(|data_type| data_type.media_type_str() == Some(kind))
                                .unwrap_or(DataType::File)
                            } else {
                                DataType::File
                            };
                            send.blocking_send((data_type, path.to_owned()))
                                .map_err(|_| anyhow::anyhow!("File scan receiver closed"))?;
                            discovered += usize::from(path.is_file());
                            Ok(())
                        },
                    )
                    .map_err(|error| error.to_string())?;
                    Ok(discovered)
                });
                while let Some((data_type, path)) = receive.recv().await {
                    session.wait_running().await?;
                    let actual = tokio::fs::canonicalize(&path)
                        .await
                        .map_err(|error| error.to_string())?;
                    if !actual.starts_with(&canonical_root) {
                        return Err("Media file escaped scan directory".into());
                    }
                    if data_type == DataType::File {
                        let file_count = usize::from(actual.is_file());
                        let path = actual
                            .to_str()
                            .ok_or("Invalid file path encoding")?
                            .to_owned();
                        let types = session
                            .record_when_running(file_count, || {
                                crate::services::files::media_index::record_generic(
                                    &self.state.db,
                                    &[path],
                                )
                            })
                            .await?;
                        self.state.media_scans.publish_catalog_types(types);
                        continue;
                    }
                    let metadata = self
                        .read_file(
                            data_type,
                            actual.to_str().ok_or("Invalid media file path encoding")?,
                        )
                        .await?;
                    if !std::path::Path::new(&metadata.metadata.path).starts_with(&canonical_root) {
                        return Err("Media file escaped scan directory".into());
                    }
                    session
                        .record_when_running(1, || self.persist_file_reads(&[metadata]))
                        .await?;
                }
                let total = producer.await.map_err(|error| error.to_string())??;
                completed_roots.push(canonical_root);

                total_files = total_files
                    .checked_add(total)
                    .ok_or("Media scan file count overflow")?;
            }
            let mut missing = Vec::new();
            for root in completed_roots {
                if !tokio::fs::metadata(&root)
                    .await
                    .map_err(|error| error.to_string())?
                    .is_dir()
                {
                    return Err("Media source became unavailable during reconciliation".into());
                }
                for item in self.path_identities(&root)? {
                    match tokio::fs::symlink_metadata(&item.path).await {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            missing.push(item.path);
                        }
                        Err(error) => return Err(error.to_string()),
                        Ok(_) => {}
                    }
                }
            }
            session
                .record_when_running(0, || {
                    self.state
                        .db
                        .mark_file_paths_missing(&missing)
                        .map_err(|error| error.to_string())
                })
                .await?;
            session.discovered(total_files)?;
            Ok(())
        };
        let result = tokio::select! {
            biased;
            _ = crate::services::files::media_scans::wait_stop(&mut cancel) => Err("Media scan cancelled".into()),
            _ = crate::services::files::media_scans::wait_stop(&mut stop) => {
                self.state.media_scans.cancel()?;
                Err("Rust core stopped".into())
            },
            result = result => result,
        };
        session.finish(result)
    }

    #[cfg(feature = "media")]
    pub fn pause_file_scan(
        &self,
    ) -> Result<Option<crate::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.pause()
    }

    #[cfg(feature = "media")]
    pub fn resume_file_scan(
        &self,
    ) -> Result<Option<crate::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.resume()
    }

    #[cfg(feature = "media")]
    pub fn cancel_file_scan(
        &self,
    ) -> Result<Option<crate::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.cancel()
    }

    #[cfg(feature = "media")]
    pub fn file_scan_status(
        &self,
    ) -> Result<Option<crate::services::files::media_scans::Snapshot>, String> {
        self.state.native_client_id()?;
        self.state.media_scans.snapshot()
    }

    pub fn record_files(&self, files: &[MediaFileMetadata]) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        let rows = self
            .state
            .db
            .record_media_files(files)
            .map_err(|error| error.to_string())?;
        crate::services::files::catalog_events::publish(
            &self.state.events,
            rows.iter().map(|row| row.data_type),
        );
        Ok(rows)
    }

    pub fn record_file_reads(
        &self,
        files: &[plain_rs::db::MediaFileMetadataRead],
    ) -> Result<Vec<MediaFileRow>, String> {
        let rows = self.persist_file_reads(files)?;
        crate::services::files::catalog_events::publish(
            &self.state.events,
            rows.iter().map(|row| row.data_type),
        );
        Ok(rows)
    }

    fn persist_file_reads(
        &self,
        files: &[plain_rs::db::MediaFileMetadataRead],
    ) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .record_media_file_reads(files)
            .map_err(|error| error.to_string())
    }

    #[cfg(any(test, feature = "media"))]
    fn forget_path(&self, path: &str) -> Result<usize, String> {
        self.state.native_client_id()?;
        let types = self
            .state
            .db
            .forget_media_path_types(path)
            .map_err(|error| error.to_string())?;
        let count = types.len();
        crate::services::files::catalog_events::publish(&self.state.events, types);
        Ok(count)
    }

    pub fn by_path(&self, data_type: DataType, path: &str) -> Result<Option<MediaFileRow>, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .media_file_by_path(data_type, path)
            .map_err(|error| error.to_string())
    }

    pub fn list(
        &self,
        data_type: DataType,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .media_file_list(data_type, limit, offset)
            .map_err(|error| error.to_string())
    }

    pub fn search(
        &self,
        data_type: DataType,
        query: &str,
        sort: crate::graphql::resolvers::files::types::FileSortBy,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MediaFileRow>, String> {
        self.state.native_client_id()?;
        crate::services::files::media_catalog::rows(
            &self.state.db,
            data_type,
            query,
            sort,
            limit,
            offset,
        )
        .map_err(|error| error.to_string())
    }

    pub fn count_matching(&self, data_type: DataType, query: &str) -> Result<i64, String> {
        self.state.native_client_id()?;
        crate::services::files::media_catalog::count(&self.state.db, data_type, query)
            .map_err(|error| error.to_string())
    }

    pub fn count(&self, data_type: DataType) -> Result<i64, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .media_file_count(data_type)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/native/media_catalog.rs"]
mod tests;
