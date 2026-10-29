use crate::db::{Db, files::PathChange};
use crate::platform::mountinfo::{self, CleanPath};
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock as Lazy, Mutex};

#[cfg(unix)]
fn owner_mode(meta: &std::fs::Metadata) -> (u32, u32, u32) {
    (meta.uid(), meta.gid(), meta.mode())
}
#[cfg(not(unix))]
fn owner_mode(_meta: &std::fs::Metadata) -> (u32, u32, u32) {
    (0, 0, 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub original_path: String,
    pub disk: String,
    pub trash_rel_path: String,
    pub deleted_at: i64,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    pub size: Option<i64>,
    pub entry_count: Option<i64>,
}

pub struct Trash {
    db: Arc<Db>,
}

const TOP_LEVEL: &str = "trashed=1 AND is_available=1 AND missing=0 AND NOT EXISTS(SELECT 1 FROM file_catalog parent WHERE parent.volume_id=file_catalog.volume_id AND parent.trashed=1 AND parent.file_id<>file_catalog.file_id AND substr(file_catalog.path,1,length(parent.path)+1)=parent.path||'/')";

fn trash_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<TrashItem> {
    let path: String = row.get(1)?;
    let absolute = Path::new(&path);
    let root = absolute
        .ancestors()
        .find(|part| part.file_name().is_some_and(|name| name == ".plain-trash"))
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let disk = root
        .parent()
        .and_then(Path::to_str)
        .ok_or(rusqlite::Error::InvalidQuery)?
        .to_owned();
    let relative = absolute
        .strip_prefix(root)
        .map_err(crate::db::files::sql_error)?
        .to_str()
        .ok_or(rusqlite::Error::InvalidQuery)?
        .to_owned();
    let time: String = row.get(4)?;
    let deleted_at = DateTime::parse_from_rfc3339(&time)
        .map_err(crate::db::files::sql_error)?
        .timestamp();
    let (uid, gid, mode) = std::fs::symlink_metadata(absolute)
        .map(|metadata| owner_mode(&metadata))
        .unwrap_or_default();
    Ok(TrashItem {
        id: row.get(0)?,
        kind: if row.get::<_, String>(3)? == "DIRECTORY" {
            "dir"
        } else {
            "file"
        }
        .into(),
        original_path: row.get(2)?,
        disk,
        trash_rel_path: relative,
        deleted_at,
        uid,
        gid,
        mode,
        size: row.get(5)?,
        entry_count: None,
    })
}

impl Trash {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub fn load_item(&self, id: &str) -> Result<Option<TrashItem>> {
        Ok(self.db.with_conn(|conn|conn.query_row("SELECT id,path,original_path,kind,trashed_at,size_bytes FROM file_catalog WHERE id=?1 AND trashed=1",[id],trash_item).optional())?)
    }

    pub async fn trash_paths(&self, paths: Vec<String>) -> Result<Vec<String>> {
        let mut result = Vec::with_capacity(paths.len());
        for raw in paths {
            let source = crate::db::files::locate(&raw)?.path;
            validate_trashable_path(&source)?;
            let metadata = std::fs::symlink_metadata(&source)?;
            let source_text = source.to_str().context("Invalid file path encoding")?;
            let file = self
                .db
                .record_file_paths(&[source_text.into()], None)?
                .remove(0);
            let kind = if metadata.is_dir() { "dir" } else { "file" };
            let mount =
                mountinfo::resolve_mount_point(source.parent().context("File parent required")?)?;
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .context("File name required")?;
            let relative = compute_bucket_rel_path(kind, &file.id.to_string(), name, Utc::now());
            let disk = writable_disk(&source, &mount, |disk| {
                prepare_trash_parent(disk, &relative)
            })?;
            let destination = abs_trash_path(&disk, &relative);
            let target = destination
                .to_str()
                .context("Invalid trash path encoding")?;
            let lock = lock_for_disk(&disk);
            let _guard = lock.lock().await;
            self.db
                .prepare_file_path_change(file.id, source_text, target, PathChange::Trash)?;
            if let Err(error) = crate::files::filesystem::rename::no_replace(&source, &destination)
            {
                self.db
                    .key_values()
                    .remove(format!("file_path_change:{}", file.id))?;
                return Err(error.into());
            }
            self.db
                .finish_file_path_change(file.id, source_text, target, PathChange::Trash)?;
            result.push(target.into());
        }
        Ok(result)
    }

    pub async fn restore_paths(&self, paths: Vec<String>) -> Result<Vec<String>> {
        let mut result = Vec::with_capacity(paths.len());
        for reference in paths {
            let item = self
                .resolve_item(&reference)?
                .context("Trash item not found")?;
            let source = Self::physical_path(&item);
            let target = Path::new(&item.original_path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let target = crate::utils::unique_path::unique_sibling(target)?;
            let source_text = source.to_str().context("Invalid trash path encoding")?;
            let target_text = target.to_str().context("Invalid restore path encoding")?;
            let lock = lock_for_disk(&item.disk);
            let _guard = lock.lock().await;
            let id = item.id.parse()?;
            self.db
                .prepare_file_path_change(id, source_text, target_text, PathChange::Restore)?;
            if let Err(error) = crate::files::filesystem::rename::no_replace(&source, &target) {
                self.db
                    .key_values()
                    .remove(format!("file_path_change:{id}"))?;
                return Err(error.into());
            }
            self.db
                .finish_file_path_change(id, source_text, target_text, PathChange::Restore)?;
            result.push(target_text.into());
        }
        Ok(result)
    }

    pub async fn delete_trash_by_path(&self, reference: &str) -> Result<()> {
        let item = self
            .resolve_item(reference)?
            .context("Trash item not found")?;
        let physical = Self::physical_path(&item);
        let lock = lock_for_disk(&item.disk);
        let _guard = lock.lock().await;
        crate::files::filesystem::remove(&physical).await?;
        let path = physical
            .to_str()
            .context("Invalid trash path encoding")?
            .to_owned();
        crate::files::deletes::cleanup(
            &self.db,
            &[],
            std::slice::from_ref(&path),
            std::slice::from_ref(&path),
        )?;
        Ok(())
    }

    pub fn resolve_item(&self, reference: &str) -> Result<Option<TrashItem>> {
        let id = validate_restore_path(Path::new(reference))?;
        let item = self.load_item(&id)?;
        if Path::new(reference).is_absolute()
            && item
                .as_ref()
                .is_some_and(|item| Self::physical_path(item) != Path::new(reference).clean_path())
        {
            bail!("Trash path does not match file identity");
        }
        Ok(item)
    }

    pub fn physical_path(item: &TrashItem) -> PathBuf {
        abs_trash_path(&item.disk, &item.trash_rel_path)
    }

    pub fn list_trash(
        &self,
        offset: usize,
        limit: usize,
        text: &str,
        order: SortOrder,
    ) -> Result<Vec<TrashItem>> {
        let needle = text.trim().to_lowercase();
        let mut items=self.db.with_conn(|conn|conn.prepare(&format!("SELECT id,path,original_path,kind,trashed_at,size_bytes FROM file_catalog WHERE {TOP_LEVEL}"))?
            .query_map([],trash_item)?.collect::<rusqlite::Result<Vec<_>>>())?;
        items.retain(|item| item.original_path.to_lowercase().contains(&needle));
        let name = |item: &TrashItem| {
            Path::new(&item.original_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_lowercase()
                .trim()
                .to_owned()
        };
        items.sort_by(|a, b| {
            let directories = (b.kind == "dir").cmp(&(a.kind == "dir"));
            let order = match order {
                SortOrder::DeletedAtNewest => b.deleted_at.cmp(&a.deleted_at),
                SortOrder::DeletedAtOldest => a.deleted_at.cmp(&b.deleted_at),
                SortOrder::NameAsc => directories.then_with(|| name(a).cmp(&name(b))),
                SortOrder::NameDesc => directories.then_with(|| name(b).cmp(&name(a))),
                SortOrder::SizeAsc => {
                    directories.then(a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)))
                }
                SortOrder::SizeDesc => {
                    directories.then(b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)))
                }
            };
            order.then(a.id.cmp(&b.id))
        });
        Ok(items.into_iter().skip(offset).take(limit).collect())
    }

    pub fn trash_count(&self) -> Result<usize> {
        let total = self.db.with_conn(|conn| {
            conn.query_row(
                &format!("SELECT count(*) FROM file_catalog WHERE {TOP_LEVEL}"),
                [],
                |row| row.get::<_, i64>(0),
            )
        })?;
        Ok(usize::try_from(total)?)
    }
}

static LOCKS: Lazy<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn lock_for_disk(disk: &str) -> Arc<tokio::sync::Mutex<()>> {
    let mut g = LOCKS.lock().expect("trash LOCKS poisoned");
    g.entry(disk.to_string())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

fn trash_root(disk_mount: &str) -> PathBuf {
    PathBuf::from(disk_mount).join(".plain-trash")
}
fn abs_trash_path(disk: &str, rel: &str) -> PathBuf {
    trash_root(disk).join(rel)
}

fn prepare_trash_parent(disk: &str, relative: &str) -> std::io::Result<()> {
    let root = trash_root(disk);
    match std::fs::create_dir(&root) {
        Ok(()) => {
            #[cfg(unix)]
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = std::fs::symlink_metadata(&root)?;
    if !metadata.file_type().is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Trash root must be a real directory",
        ));
    }
    #[cfg(unix)]
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Trash root must be private and owned by the current user",
        ));
    }
    let mut parent = root;
    for component in Path::new(relative)
        .parent()
        .unwrap_or(Path::new(""))
        .components()
    {
        parent.push(component.as_os_str());
        match std::fs::create_dir(&parent) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        if !std::fs::symlink_metadata(&parent)?.file_type().is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Trash parent must be a real directory",
            ));
        }
    }
    Ok(())
}

fn writable_disk(
    source: &Path,
    mount: &str,
    mut prepare: impl FnMut(&str) -> std::io::Result<()>,
) -> Result<String> {
    match prepare(mount) {
        Ok(()) => Ok(mount.to_owned()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::PermissionDenied
                    | std::io::ErrorKind::ReadOnlyFilesystem
                    | std::io::ErrorKind::NotFound
            ) =>
        {
            let parent = source
                .parent()
                .and_then(Path::to_str)
                .ok_or_else(|| anyhow!("trash parent required"))?;
            prepare(parent).with_context(|| {
                format!("cannot prepare writable trash for {}", source.display())
            })?;
            Ok(parent.to_owned())
        }
        Err(error) => Err(error.into()),
    }
}

fn compute_bucket_rel_path(
    kind: &str,
    id: &str,
    original_name: &str,
    when: DateTime<Utc>,
) -> String {
    let prefix = if kind == "dir" { "d" } else { "f" };
    let name = sanitize_bucket_name(original_name);
    format!(
        "data/{:04}/{:02}/{prefix}_{id}_{name}",
        when.format("%Y").to_string().parse::<i32>().unwrap_or(1970),
        when.format("%m").to_string().parse::<u32>().unwrap_or(1),
    )
}

fn sanitize_bucket_name(s: &str) -> String {
    s.chars()
        .filter(|c| {
            c.is_ascii_alphanumeric() || matches!(*c, '.' | '-' | '_' | '+' | ' ' | '(' | ')')
        })
        .collect()
}

fn is_in_plain_trash<P: AsRef<Path>>(p: P) -> bool {
    p.as_ref()
        .components()
        .any(|c| c.as_os_str() == ".plain-trash")
}

pub fn is_trashed_path(p: &str) -> bool {
    is_in_plain_trash(PathBuf::from(p).clean_path())
}

fn validate_trashable_path(p: &Path) -> Result<()> {
    let cleaned = p.clean_path();
    if cleaned.as_os_str().is_empty() || cleaned == PathBuf::from(".") {
        bail!("invalid path");
    }
    if cleaned == PathBuf::from("/") {
        bail!("refuse to trash root");
    }
    if is_in_plain_trash(&cleaned) {
        bail!("refuse to trash items inside .plain-trash");
    }
    Ok(())
}

fn validate_restore_path(p: &Path) -> Result<String> {
    let cleaned = p.clean_path();
    if is_in_plain_trash(&cleaned) {
        let id = parse_trash_id_from_path(&cleaned).ok_or_else(|| anyhow!("invalid trash path"))?;
        Ok(id)
    } else {
        let s = cleaned.to_string_lossy().to_string();
        if s.trim().is_empty() {
            bail!("invalid trash id");
        }
        if s.contains('/') {
            bail!("invalid trash id");
        }
        Ok(s)
    }
}

fn parse_trash_id_from_path(p: &Path) -> Option<String> {
    let name = p.file_name()?.to_str()?;
    let rest = if let Some(s) = name.strip_prefix("f_") {
        s
    } else if let Some(s) = name.strip_prefix("d_") {
        s
    } else {
        return None;
    };
    let id = rest.split('_').next()?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    DeletedAtNewest,
    DeletedAtOldest,
    NameAsc,
    NameDesc,
    SizeAsc,
    SizeDesc,
}

pub fn fill_stats(it: &mut TrashItem) -> Result<()> {
    let abs = abs_trash_path(&it.disk, &it.trash_rel_path);
    if it.kind == "dir" {
        if abs.exists() {
            it.size = Some(dir_size(&abs));
            it.entry_count = Some(count_entries(&abs));
        } else {
            it.size = Some(0);
            it.entry_count = Some(0);
        }
    } else if abs.exists() {
        if let Ok(m) = std::fs::symlink_metadata(&abs) {
            it.size = Some(m.len() as i64);
        }
    }
    Ok(())
}

fn dir_size(p: &Path) -> i64 {
    crate::files::filesystem::walk::Walk::new(p)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len() as i64)
        .sum()
}
fn count_entries(p: &Path) -> i64 {
    crate::files::filesystem::walk::Walk::new(p)
        .into_iter()
        .filter_map(|e| e.ok())
        .count() as i64
}

#[cfg(test)]
#[path = "../../tests/unit/files/trash.rs"]
mod tests;
