use crate::features::access::AccessFeature;
mod media_rows;
pub(super) mod paths;

use crate::features::files::tasks::{FileTaskOp, FileTaskType};
use crate::infrastructure::db::Db;
use crate::infrastructure::host::bridge::Host;
use anyhow::{Result, anyhow, bail};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct FilePlatform {
    pub photos: super::photo_catalog::PhotoCatalog,
    pub(super) host: Arc<Host>,
    pub(super) db: Arc<Db>,
    pub recent_files: Arc<crate::features::recent_files::RecentFiles>,
    directory: PathBuf,
    cache: PathBuf,
    pub(super) private_roots: std::result::Result<[PathBuf; 2], String>,
    #[cfg(not(feature = "runtime-media"))]
    events: tokio::sync::broadcast::Sender<crate::infrastructure::events::WsEvent>,
    #[cfg(any(test, feature = "test-seams"))]
    sdk: std::sync::atomic::AtomicBool,
    #[cfg(feature = "runtime-media")]
    scans: Arc<crate::features::services::files::media_scans::MediaScans>,
}
impl FilePlatform {
    pub fn new(
        host: Arc<Host>,
        db: Arc<Db>,
        directory: PathBuf,
        cache: PathBuf,
        #[cfg(not(feature = "runtime-media"))] events: tokio::sync::broadcast::Sender<
            crate::infrastructure::events::WsEvent,
        >,
        #[cfg(feature = "runtime-media")] scans: Arc<
            crate::features::services::files::media_scans::MediaScans,
        >,
    ) -> Self {
        let private_roots =
            (|| -> std::io::Result<_> { Ok([directory.canonicalize()?, cache.canonicalize()?]) })()
                .map_err(|error| error.to_string());
        Self {
            photos: super::photo_catalog::PhotoCatalog::default(),
            recent_files: Arc::new(crate::features::recent_files::RecentFiles::new(
                db.key_values(),
            )),
            private_roots,
            host,
            db,
            directory,
            cache,
            #[cfg(not(feature = "runtime-media"))]
            events,
            #[cfg(any(test, feature = "test-seams"))]
            sdk: std::sync::atomic::AtomicBool::new(cfg!(any(
                target_os = "android",
                target_os = "ios"
            ))),
            #[cfg(feature = "runtime-media")]
            scans,
        }
    }
    pub(crate) async fn resource_media_path(&self, id: &str) -> Result<String> {
        if self.host.uses_system_media_catalog() {
            let mut plan = crate::features::services::files::provider_query::Plan::default();
            plan.ids("_id", vec![id.to_owned()]);
            let rows = self
                .host
                .call(
                    "systemFileIndexRows",
                    json!({"plan":plan,"offset":0,"limit":1,"sortBy":"NAME_ASC"}),
                )
                .await
                .map_err(anyhow::Error::msg)?;
            return rows
                .as_array()
                .and_then(|rows| rows.first())
                .filter(|row| row["mediaId"].as_str() == Some(id))
                .and_then(|row| row["path"].as_str())
                .filter(|path| !path.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("Media unavailable"));
        }
        self.db.media_resource_path(id)
    }

    pub fn uses_sdk(&self) -> bool {
        #[cfg(any(test, feature = "test-seams"))]
        {
            self.sdk.load(std::sync::atomic::Ordering::Relaxed)
        }
        #[cfg(not(any(test, feature = "test-seams")))]
        {
            cfg!(any(target_os = "android", target_os = "ios"))
        }
    }

    pub async fn mounts(
        &self,
        prefs: Arc<crate::features::prefs::Prefs>,
    ) -> Result<serde_json::Value> {
        if self.uses_sdk() {
            return self
                .host
                .call(
                    crate::infrastructure::host::commands::SYSTEM_MOUNT_FACTS,
                    json!({}),
                )
                .await
                .map_err(anyhow::Error::msg);
        }
        tokio::task::spawn_blocking(move || {
            crate::features::services::files::mounts::native_facts(&prefs)
        })
        .await?
        .map_err(anyhow::Error::msg)
    }

    pub fn watchable(&self, path: &Path) -> bool {
        let Ok(private) = &self.private_roots else {
            return false;
        };
        if !path.is_absolute() || private.iter().any(|root| path.starts_with(root)) {
            return false;
        }
        let parts = path
            .components()
            .filter_map(|part| part.as_os_str().to_str())
            .collect::<Vec<_>>();
        !parts.iter().any(|part| part.starts_with('.'))
            && !parts
                .windows(2)
                .any(|parts| parts[0] == "Android" && matches!(parts[1], "data" | "obb"))
    }

    pub(super) fn indexable_type(&self, path: &Path) -> Option<&'static str> {
        let name = path.to_str()?;
        if !self.watchable(path)
            || crate::features::media::catalog_paths::is_media_excluded_at(
                name, &self.directory, &self.cache, &[],
            )
        {
            return None;
        }
        let kind = crate::features::media::catalog_paths::infer_type(name);
        (kind != "other").then_some(kind)
    }

    pub fn resolve_reference(&self, path: &str) -> Result<String> {
        crate::features::services::files::references::resolve(
            self.db.clone(),
            self.directory.clone(),
            path,
        )
    }

    #[cfg(any(test, feature = "test-seams"))]
    pub fn use_sdk_for_test(&self) {
        self.sdk.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn thumbnail_cache(&self, path: &str) -> Result<PathBuf> {
        if !Path::new(path).is_absolute() {
            bail!("absolute thumbnail path required");
        }
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() {
            bail!("regular thumbnail source required");
        }
        let _file = std::fs::File::open(path)?;
        Ok(self.cache.clone())
    }

    pub async fn recent_files(&self) -> Result<Vec<serde_json::Value>> {
        if self.uses_sdk() {
            if self.host.startup().recent_files_catalog {
                let mut plan = crate::features::services::files::provider_query::Plan::default();
                plan.add(
                    "mime_type != ?".into(),
                    vec!["vnd.android.document/directory".into()],
                );
                return serde_json::from_value(
                    self.host
                        .call(
                            "systemFileIndexRows",
                            json!({
                                "plan":plan,"offset":0,"limit":100,"sortBy":"DATE_DESC"
                            }),
                        )
                        .await
                        .map_err(anyhow::Error::msg)?,
                )
                .map_err(Into::into);
            }
            let volumes = self
                .host
                .call(
                    crate::infrastructure::host::commands::SYSTEM_MOUNT_FACTS,
                    json!({}),
                )
                .await
                .map_err(anyhow::Error::msg)?;
            let root = volumes
                .as_array()
                .ok_or_else(|| anyhow!("Invalid storage volumes"))?
                .iter()
                .find(|volume| volume["driveType"] == "INTERNAL_STORAGE")
                .and_then(|volume| volume["path"].as_str())
                .ok_or_else(|| anyhow!("Primary storage directory unavailable"))?;
            if !Path::new(root).is_absolute() {
                bail!("Absolute primary storage directory required");
            }
            let root = PathBuf::from(root);
            let entries = tokio::task::spawn_blocking(move || {
                plain_libs::filesystem::recent::list(&root, 100)
            })
            .await?;
            return Ok(entries.into_iter().map(|entry| json!({
                "name":Path::new(&entry.path).file_name().and_then(|name| name.to_str()).unwrap_or_default(),
                "path":entry.path,"permission":"","mediaId":null,"createdAt":null,
                "updatedAt":entry.updated_at.timestamp_millis(),"size":entry.size,
                "isDir":false,"childCount":0
            })).collect());
        }
        let mut items = Vec::new();
        for path in self.recent_files.list(500)? {
            let Ok(entry) = plain_libs::filesystem::stat(Path::new(&path)).await else {
                continue;
            };
            items.push(json!({"name":Path::new(&entry.path).file_name().and_then(|name| name.to_str()).unwrap_or_default(),
                "path":entry.path,"permission":"","mediaId":null,"createdAt":entry.created_at.timestamp_millis(),
                "updatedAt":entry.updated_at.timestamp_millis(),"size":entry.size,"isDir":entry.is_dir,"childCount":entry.child_count}));
        }
        Ok(items)
    }

    pub async fn indexed_files(
        &self,
        query: String,
        offset: i32,
        limit: i32,
        sort: crate::features::prefs::settings::FileSortBy,
        count_only: bool,
    ) -> Result<serde_json::Value> {
        let limit = limit.clamp(0, super::browse::SEARCH_RESULT_LIMIT as i32);
        if self.host.uses_system_media_catalog() {
            return crate::features::services::files::indexed::platform(
                &self.db, &self.host, &query, offset, limit, sort, count_only,
            )
            .await;
        }
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            crate::features::services::files::indexed::native(
                &db, &query, offset, limit, sort, count_only,
            )
        })
        .await?
    }
    async fn receipt(&self, method: &str, params: serde_json::Value) -> Result<()> {
        if self
            .host
            .call(method, params)
            .await
            .map_err(anyhow::Error::msg)?
            .as_bool()
            != Some(true)
        {
            bail!("invalid file platform receipt");
        }
        Ok(())
    }
    pub async fn authorize_task(&self, kind: FileTaskType, ops: &[FileTaskOp]) -> Result<()> {
        if self.uses_sdk() {
            let sources = ops.iter().map(|op| op.src.clone()).collect::<Vec<_>>();
            let destinations = ops.iter().map(|op| op.dst.clone()).collect::<Vec<_>>();
            let source_external = self
                .external_paths(&sources, kind == FileTaskType::Move)
                .await?;
            let destination_external = self.external_paths(&destinations, true).await?;
            return self
                .require_external_permission(
                    !source_external.is_empty() || !destination_external.is_empty(),
                )
                .await;
        }
        for op in ops {
            self.authorize(&op.src, kind == FileTaskType::Move).await?;
            self.authorize(&op.dst, true).await?;
        }
        Ok(())
    }
    pub async fn authorize(&self, path: &str, write: bool) -> Result<()> {
        self.authorize_paths(&[path.to_owned()], write).await
    }
    pub async fn authorize_paths(&self, paths: &[String], write: bool) -> Result<()> {
        if paths.iter().any(|path| !Path::new(path).is_absolute()) {
            bail!("absolute file path required");
        }
        if self.uses_sdk() {
            let external = self.external_paths(paths, write).await?;
            return self.require_external_permission(!external.is_empty()).await;
        }
        let paths = paths.to_vec();
        tokio::task::spawn_blocking(move || {
            for path in paths {
                authorize_native(Path::new(&path), write)?;
            }
            Ok(())
        })
        .await?
    }
    pub async fn authorize_metadata_paths(&self, paths: &[String]) -> Result<()> {
        if paths.iter().any(|path| !Path::new(path).is_absolute()) {
            bail!("absolute file path required");
        }
        if self.uses_sdk() {
            self.authorize_paths(paths, false).await?;
        }
        Ok(())
    }

    async fn require_external_permission(&self, needed: bool) -> Result<()> {
        if needed
            && !crate::features::services::device::access::granted(
                &self.host,
                &[AccessFeature::Files],
            )
            .await?
        {
            bail!("File access permission denied");
        }
        Ok(())
    }

    async fn external_paths(&self, paths: &[String], write: bool) -> Result<Vec<String>> {
        let paths = paths.to_vec();
        let [directory, cache] = self
            .private_roots
            .as_ref()
            .map_err(|error| anyhow!("{error}"))?
            .clone();
        tokio::task::spawn_blocking(move || paths::external(&paths, &directory, &cache, write))
            .await?
    }

    pub async fn identities(
        &self,
        paths: &[String],
    ) -> Result<Vec<crate::features::services::files::file_task_media::Identity>> {
        if self.host.uses_system_media_catalog() {
            let paths = self.external_paths(paths, false).await?;
            if paths.is_empty() {
                return Ok(vec![]);
            }
            let rows = media_rows::identities(
                self.host
                    .call("fileTaskMediaSnapshot", json!({"paths": paths}))
                    .await
                    .map_err(anyhow::Error::msg)?,
            )?;
            crate::features::services::files::file_task_media::validate(&rows, &paths)?;
            return Ok(rows);
        }
        let db = self.db.clone();
        let paths = serde_json::to_string(paths)?;
        Ok(tokio::task::spawn_blocking(move || db.with_conn(|connection| {
            connection.prepare("SELECT data_type,id,path FROM file_catalog WHERE missing=0 AND is_available=1 AND data_type IN (1,2,3,22,24) AND path IN (SELECT value FROM json_each(?1))")?
                .query_map([paths], |row| Ok(crate::features::services::files::file_task_media::Identity { media_type: row.get(0)?, file_id: row.get(1)?, path: row.get(2)? }))?
                .collect::<rusqlite::Result<Vec<_>>>()
        })).await??)
    }
    pub async fn scan(
        &self,
        paths: &[String],
    ) -> Result<Vec<crate::infrastructure::db::MediaFileRow>> {
        if self.host.uses_system_media_catalog() {
            let paths = self.external_paths(paths, false).await?;
            if paths.is_empty() {
                return Ok(Vec::new());
            }
            self.receipt("fileTaskScan", json!({"paths":paths})).await?;
            return Ok(Vec::new());
        }
        #[cfg(feature = "runtime-media")]
        {
            let paths = paths.to_vec();
            let existing = tokio::task::spawn_blocking(move || {
                let mut existing = Vec::new();
                for path in paths {
                    match std::fs::metadata(&path) {
                        Ok(metadata) if metadata.is_file() || metadata.is_dir() => {
                            existing.push(path)
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.to_string()),
                    }
                }
                Ok::<_, String>(existing)
            })
            .await?
            .map_err(anyhow::Error::msg)?;
            crate::features::services::files::native_scan::completed(
                &self.db,
                &self.scans,
                &self.directory,
                existing,
            )
            .await
            .map_err(anyhow::Error::msg)
        }
        #[cfg(not(feature = "runtime-media"))]
        {
            let db = self.db.clone();
            let directory = self.directory.clone();
            let cache = self.cache.clone();
            let paths = paths.to_vec();
            let (rows, generic_types) = tokio::task::spawn_blocking(move || {
                let mut existing = Vec::new();
                for path in paths {
                    match std::fs::metadata(&path) {
                        Ok(metadata) if metadata.is_file() || metadata.is_dir() => {
                            existing.push(path)
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.to_string()),
                    }
                }
                let reads = crate::features::services::files::media_index::read_files(
                    &directory,
                    &cache,
                    &[],
                    existing,
                    None,
                )?;
                let generic_types = crate::features::services::files::media_index::record_generic(
                    &db,
                    &reads.generic,
                )?;
                let rows = db
                    .record_media_file_reads(&reads.media)
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>((rows, generic_types))
            })
            .await?
            .map_err(anyhow::Error::msg)?;
            crate::features::services::files::catalog_events::publish(
                &self.events,
                rows.iter().map(|row| row.data_type).chain(generic_types),
            );
            Ok(rows)
        }
    }
    pub async fn forget_removed(&self, paths: Vec<String>) -> Result<()> {
        if self.host.uses_system_media_catalog() {
            return Ok(());
        }
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            for path in paths {
                if !Path::new(&path).try_exists()? {
                    db.forget_media_file(&path)?;
                }
            }
            anyhow::Ok(())
        })
        .await?
    }
}
pub(super) fn authorize_native(path: &Path, write: bool) -> Result<()> {
    if !path.is_absolute() {
        bail!("absolute file path required");
    }
    if write {
        plain_libs::filesystem::deletion::validate(path)?;
        let mut ancestor = path
            .parent()
            .ok_or_else(|| anyhow!("file parent required"))?;
        while !ancestor.try_exists()? {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| anyhow!("existing file ancestor required"))?;
        }
        let ancestor = ancestor.canonicalize()?;
        if !ancestor.is_dir() {
            bail!("writable file parent required");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::CString::new(ancestor.as_os_str().as_bytes())?;
            if unsafe { libc::access(name.as_ptr(), libc::W_OK | libc::X_OK) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
    } else {
        let path = path.canonicalize()?;
        if path.is_file() {
            std::fs::File::open(path)?;
        } else if path.is_dir() {
            std::fs::read_dir(path)?;
        } else {
            bail!("regular file or directory required");
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/platform.rs"]
mod tests;
