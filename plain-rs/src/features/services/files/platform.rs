use crate::features::access::AccessFeature;
pub(super) mod media_rows;
pub(super) mod paths;

use crate::features::files::tasks::{FileTaskOp, FileTaskType};
use crate::infrastructure::db::Db;
use crate::infrastructure::host::bridge::Host;
use plain_libs::anyhow::{Result, anyhow, bail};
use plain_libs::filesystem::browse::ExcludedPaths;
use plain_libs::serde_json::json;
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
    protected_files: ExcludedPaths,
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
        let data = crate::infrastructure::data_paths::DataPaths::new(&directory);
        let database = data.database();
        let legacy_database = data.legacy_desktop_database();
        let prefs = data.prefs();
        let tls_identity = data.tls_identity();
        let default_key = directory.join("tls.key");
        let configured_key =
            crate::infrastructure::platform::system::consts::AppPaths::tls_key_for(&directory);
        let mut protected_files = vec![
            database.clone(),
            legacy_database.clone(),
            data.legacy_desktop_prefs(),
            data.tls_certificate(),
            prefs.with_extension("json.tmp"),
            prefs,
            tls_identity.with_extension("tmp"),
            tls_identity,
            default_key.with_extension("tmp"),
            default_key,
            configured_key.with_extension("tmp"),
            configured_key,
        ];
        #[cfg(target_os = "android")]
        if let Some(legacy) = &host.startup().legacy_storage {
            protected_files.extend([legacy.preferences(), legacy.backup_preferences()]);
        }
        for database in [database, legacy_database] {
            for suffix in ["-wal", "-shm", "-journal"] {
                let mut path = database.clone().into_os_string();
                path.push(suffix);
                protected_files.push(path.into());
            }
        }
        // Keep both the configured location and the filesystem's resolved path.
        for path in protected_files.clone() {
            if let Ok(path) = std::path::absolute(path)
                && let Ok(path) = paths::canonical(&path)
            {
                protected_files.push(path);
            }
        }
        protected_files.sort_unstable();
        protected_files.dedup();
        Self {
            protected_files: ExcludedPaths::new(
                protected_files.into(),
                private_roots
                    .as_ref()
                    .map(|[root, _]| root.clone())
                    .unwrap_or_else(|_| directory.clone()),
                crate::features::services::backups::restore::reserved,
            ),
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
    pub(crate) async fn resource_media_file(&self, id: &str) -> Result<super::links::File> {
        if self.host.uses_system_media_catalog() {
            let mut plan = crate::features::services::files::provider_query::Plan::default();
            plan.ids("_id", vec![id.to_owned()]);
            let rows = self
                .host
                .call(
                    "systemFileIndexRows",
                    json!({"plan":plan,"offset":0,"limit":1,"sortBy":"NAME_ASC","includeTrashed":true}),
                )
                .await
                .map_err(plain_libs::anyhow::Error::msg)?;
            let row = rows
                .as_array()
                .and_then(|rows| rows.first())
                .filter(|row| row["mediaId"].as_str() == Some(id))
                .ok_or_else(|| anyhow!("Media unavailable"))?;
            let path = row["path"]
                .as_str()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| anyhow!("Media path unavailable"))?;
            let name = row["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or_else(|| anyhow!("Media filename unavailable"))?;
            return Ok(super::links::File {
                path: path.into(),
                name: name.into(),
                media_id: id.into(),
            });
        }
        let path = self.db.media_resource_path(id)?;
        let name = Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Media filename unavailable"))?
            .to_owned();
        Ok(super::links::File {
            path,
            name,
            media_id: id.into(),
        })
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

    async fn mount_facts(&self) -> Result<plain_libs::serde_json::Value> {
        if self.uses_sdk() {
            return self
                .host
                .call(
                    crate::infrastructure::host::commands::SYSTEM_MOUNT_FACTS,
                    json!({}),
                )
                .await
                .map_err(plain_libs::anyhow::Error::msg);
        }
        let mut facts = tokio::task::spawn_blocking(super::mounts::native_facts)
            .await?
            .map_err(plain_libs::anyhow::Error::msg)?;
        let path = self
            .directory
            .to_str()
            .ok_or_else(|| anyhow!("Invalid App Data path"))?;
        facts
            .as_array_mut()
            .ok_or_else(|| anyhow!("Invalid storage volume facts"))?
            .push(json!({"path": path, "mountPoint": path, "driveType": "APP"}));
        Ok(facts)
    }

    pub async fn mounts(&self) -> Result<plain_libs::serde_json::Value> {
        let facts = self.mount_facts().await?;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || super::mounts::apply_aliases(&db, facts))
            .await?
            .map_err(plain_libs::anyhow::Error::msg)
    }

    pub async fn set_mount_alias(&self, id: String, alias: String) -> Result<()> {
        let facts = self.mount_facts().await?;
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || super::mounts::set_alias(&db, &facts, &id, &alias))
            .await?
            .map_err(plain_libs::anyhow::Error::msg)
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
                name,
                &self.directory,
                &self.cache,
                &[],
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

    pub async fn recent_files(&self) -> Result<Vec<plain_libs::serde_json::Value>> {
        if self.uses_sdk() {
            if self.host.startup().recent_files_catalog {
                let mut plan = crate::features::services::files::provider_query::Plan::default();
                plan.add(
                    "mime_type != ?".into(),
                    vec!["vnd.android.document/directory".into()],
                );
                return plain_libs::serde_json::from_value(
                    self.host
                        .call(
                            "systemFileIndexRows",
                            json!({
                                "plan":plan,"offset":0,"limit":100,"sortBy":"UPDATED_AT_DESC"
                            }),
                        )
                        .await
                        .map_err(plain_libs::anyhow::Error::msg)?,
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
                .map_err(plain_libs::anyhow::Error::msg)?;
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
    ) -> Result<plain_libs::serde_json::Value> {
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
    async fn receipt(&self, method: &str, params: plain_libs::serde_json::Value) -> Result<()> {
        if self
            .host
            .call(method, params)
            .await
            .map_err(plain_libs::anyhow::Error::msg)?
            .as_bool()
            != Some(true)
        {
            bail!("invalid file platform receipt");
        }
        Ok(())
    }
    pub(super) fn protected_files(&self) -> ExcludedPaths {
        self.protected_files.clone()
    }

    pub(super) fn is_protected_file(&self, path: &Path) -> bool {
        self.protected_files.contains(path)
    }

    fn check_protected_path(
        files: &ExcludedPaths,
        root: &Path,
        path: &Path,
        write: bool,
        recursive: bool,
    ) -> Result<()> {
        let resolved = paths::canonical(path)?;
        if files.contains(path)
            || files.contains(&resolved)
            || (recursive && files.contains_files_under(&resolved))
        {
            bail!("Private application file");
        }
        if write && (resolved == root || (recursive && root.starts_with(&resolved))) {
            bail!("App Data cannot be moved or deleted");
        }
        Ok(())
    }

    pub(super) async fn protect_tree(&self, path: &str, write: bool) -> Result<()> {
        let [root, _] = self
            .private_roots
            .as_ref()
            .map_err(|error| anyhow!("{error}"))?;
        let root = root.clone();
        let path = PathBuf::from(path);
        let files = self.protected_files();
        tokio::task::spawn_blocking(move || {
            Self::check_protected_path(&files, &root, &path, write, true)
        })
        .await?
    }

    pub async fn authorize_task(&self, kind: FileTaskType, ops: &[FileTaskOp]) -> Result<()> {
        for op in ops {
            self.protect_tree(&op.src, kind == FileTaskType::Move)
                .await?;
            self.protect_tree(&op.dst, true).await?;
        }
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
        let sdk = self.uses_sdk();
        self.check_paths(paths, write, !sdk).await?;
        if sdk {
            let external = self.external_paths(paths, write).await?;
            self.require_external_permission(!external.is_empty())
                .await?;
        }
        Ok(())
    }

    pub async fn authorize_metadata_paths(&self, paths: &[String]) -> Result<()> {
        if self.uses_sdk() {
            self.authorize_paths(paths, false).await
        } else {
            self.check_paths(paths, false, false).await
        }
    }

    async fn check_paths(&self, paths: &[String], write: bool, native_access: bool) -> Result<()> {
        let [root, _] = self
            .private_roots
            .as_ref()
            .map_err(|error| anyhow!("{error}"))?;
        let root = root.clone();
        let paths = paths.to_vec();
        let files = self.protected_files();
        tokio::task::spawn_blocking(move || {
            for path in &paths {
                let path = Path::new(path);
                Self::check_protected_path(&files, &root, path, write, false)?;
                if native_access {
                    authorize_native(path, write)?;
                }
            }
            Ok(())
        })
        .await?
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
                    .map_err(plain_libs::anyhow::Error::msg)?,
            )?;
            crate::features::services::files::file_task_media::validate(&rows, &paths)?;
            return Ok(rows);
        }
        let db = self.db.clone();
        let paths = plain_libs::serde_json::to_string(paths)?;
        Ok(tokio::task::spawn_blocking(move || db.with_conn(|connection| {
            connection.prepare("SELECT data_type,id,path FROM file_catalog WHERE missing=0 AND is_available=1 AND data_type IN (1,2,3,22,24) AND path IN (SELECT value FROM json_each(?1))")?
                .query_map([paths], |row| Ok(crate::features::services::files::file_task_media::Identity { media_type: row.get(0)?, file_id: row.get(1)?, path: row.get(2)? }))?
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
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
            .map_err(plain_libs::anyhow::Error::msg)?;
            crate::features::services::files::native_scan::completed(
                &self.db,
                &self.scans,
                &self.directory,
                existing,
            )
            .await
            .map_err(plain_libs::anyhow::Error::msg)
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
            .map_err(plain_libs::anyhow::Error::msg)?;
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
            Ok::<(), plain_libs::anyhow::Error>(())
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
