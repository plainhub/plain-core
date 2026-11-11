use crate::infrastructure::db::Db;
use crate::infrastructure::db::FileKind;
use crate::infrastructure::db::FileRow;
use crate::infrastructure::db::VolumeRow;
use plain_libs::platform::volumes;
use plain_libs::platform::volumes::Location;
use plain_libs::platform::volumes::Volume;
use plain_libs::rusqlite::{Connection, OptionalExtension, params};
use std::path::{Component, Path};

pub const COLUMNS: &str = "id,volume_id,path,file_name,kind,file_identity,mime_type,size_bytes,created_at,updated_at,missing,favorited,trashed_at,original_path,duration_ms,width,height,rotation,taken_at,title,artist,album,album_artist,album_id,cover_image_id,indexed";

pub fn name_filter(text: &str) -> (String, Vec<String>) {
    if text.contains('\0') {
        return ("0=1".into(), Vec::new());
    }
    let literal = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let exact = "file_name LIKE '%'||?||'%' ESCAPE '\\'";
    if text.chars().count() < 3 {
        (exact.into(), vec![literal])
    } else {
        (
            format!(
                "file_id IN (SELECT files.id FROM file_names_fts JOIN files ON files.rowid=file_names_fts.rowid WHERE file_names_fts MATCH ?) AND {exact}"
            ),
            vec![format!("\"{}\"", text.replace('"', "\"\"")), literal],
        )
    }
}

pub fn from_row(row: &plain_libs::rusqlite::Row<'_>) -> plain_libs::rusqlite::Result<FileRow> {
    Ok(FileRow {
        id: row.get(0)?,
        volume_id: row.get(1)?,
        path: row.get(2)?,
        file_name: row.get(3)?,
        kind: row.get(4)?,
        file_identity: row.get(5)?,
        mime_type: row.get(6)?,
        size_bytes: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        missing: row.get(10)?,
        favorited: row.get(11)?,
        trashed_at: row.get(12)?,
        original_path: row.get(13)?,
        duration_ms: row.get(14)?,
        width: row.get(15)?,
        height: row.get(16)?,
        rotation: row.get(17)?,
        taken_at: row.get(18)?,
        title: row.get(19)?,
        artist: row.get(20)?,
        album: row.get(21)?,
        album_artist: row.get(22)?,
        album_id: row.get(23)?,
        cover_image_id: row.get(24)?,
        indexed: row.get(25)?,
    })
}

pub(crate) fn sql_error(
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> plain_libs::rusqlite::Error {
    plain_libs::rusqlite::Error::ToSqlConversionFailure(error.into())
}

pub fn relative_path(path: &Path) -> plain_libs::rusqlite::Result<String> {
    path.components()
        .map(|part| match part {
            Component::Normal(value) => value
                .to_str()
                .filter(|value| !value.contains(['\\', '\0']))
                .map(str::to_owned)
                .ok_or(plain_libs::rusqlite::Error::InvalidQuery),
            _ => Err(plain_libs::rusqlite::Error::InvalidQuery),
        })
        .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

fn path_text(path: &Path) -> plain_libs::rusqlite::Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(plain_libs::rusqlite::Error::InvalidQuery)
}

pub(crate) fn locate(path: &str) -> plain_libs::rusqlite::Result<Location> {
    volumes::locate(Path::new(path)).map_err(|error| sql_error(error.into_boxed_dyn_error()))
}

fn available_location(path: &Path) -> plain_libs::rusqlite::Result<Option<Location>> {
    match volumes::locate(path) {
        Ok(location) => Ok(Some(location)),
        Err(error) if error.is::<volumes::Unavailable>() => Ok(None),
        Err(error) => Err(sql_error(error.into_boxed_dyn_error())),
    }
}

fn root_is_current(conn: &Connection, id: &str, old: &Volume) -> plain_libs::rusqlite::Result<bool> {
    let Some(current) = available_location(&old.path)? else {
        return Ok(false);
    };
    if current.volume == *old {
        return Ok(true);
    }
    let mut paths = conn.prepare("SELECT path FROM files WHERE volume_id=?1 ORDER BY id")?;
    for path in paths.query_map([id], |row| row.get::<_, String>(0))? {
        if available_location(&old.path.join(path?))?
            .is_some_and(|location| location.volume == *old)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn bind_volume(
    conn: &Connection,
    location: &Location,
) -> plain_libs::rusqlite::Result<(String, String)> {
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id,path FROM volumes WHERE identity=?1",
            [&location.volume.identity],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let candidate = &location.volume.path;
    let (id, root) = if let Some((id, path)) = existing {
        let old = Volume {
            identity: location.volume.identity.clone(),
            path: path.into(),
        };
        let root = if old.path == *candidate {
            old.path
        } else if root_is_current(conn, &id, &old)? && location.path.starts_with(&old.path) {
            old.path
        } else if old.path.starts_with(candidate) && root_is_current(conn, &id, &old)? {
            let prefix = relative_path(old.path.strip_prefix(candidate).map_err(sql_error)?)?;
            let (included, excluded): (String, String) = conn.query_row(
                "SELECT included_dirs,excluded_dirs FROM volumes WHERE id=?1",
                [&id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let rebase = |json: &str| -> plain_libs::rusqlite::Result<String> {
                let directories: Vec<String> = plain_libs::serde_json::from_str(json).map_err(sql_error)?;
                plain_libs::serde_json::to_string(
                    &directories
                        .into_iter()
                        .map(|path| {
                            if path.is_empty() {
                                prefix.clone()
                            } else {
                                format!("{prefix}/{path}")
                            }
                        })
                        .collect::<Vec<_>>(),
                )
                .map_err(sql_error)
            };
            conn.execute(
                "UPDATE volumes SET included_dirs=?1,excluded_dirs=?2 WHERE id=?3",
                params![rebase(&included)?, rebase(&excluded)?, id],
            )?;
            conn.execute(
                "UPDATE files SET path=?1||CASE WHEN path='' THEN '' ELSE '/'||path END,original_path=CASE WHEN original_path IS NULL THEN NULL ELSE ?1||CASE WHEN original_path='' THEN '' ELSE '/'||original_path END END WHERE volume_id=?2",
                params![prefix,id],
            )?;
            candidate.clone()
        } else {
            candidate.clone()
        };
        conn.execute(
            "UPDATE volumes SET path=?1,is_available=1 WHERE id=?2 AND (path IS NOT ?1 OR is_available<>1)",
            params![path_text(&root)?, id],
        )?;
        (id, root)
    } else {
        let id = crate::features::record_id::generate();
        conn.execute(
            "INSERT INTO volumes(id,identity,path,is_available) VALUES(?1,?2,?3,1)",
            params![id, location.volume.identity, path_text(candidate)?],
        )?;
        (id, candidate.clone())
    };
    Ok((
        id,
        relative_path(location.path.strip_prefix(root).map_err(sql_error)?)?,
    ))
}

pub(crate) fn upsert(
    conn: &Connection,
    location: &Location,
    kind: Option<FileKind>,
    identity: Option<&str>,
    size: Option<i64>,
    created: Option<&str>,
    updated: &str,
    scan_id: Option<&str>,
) -> plain_libs::rusqlite::Result<String> {
    let (volume_id, path) = bind_volume(conn, location)?;
    let existing: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT id,file_identity FROM files WHERE volume_id=?1 AND path=?2 AND missing=0",
            params![volume_id, path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let mut id = None;
    if let Some((existing_id, old_identity)) = existing {
        if let (Some(old), Some(new)) = (old_identity.as_deref(), identity) {
            if old != new {
                conn.execute("UPDATE files SET missing=1 WHERE id=?1", [existing_id])?;
            } else {
                id = Some(existing_id);
            }
        } else {
            id = Some(existing_id);
        }
    }
    if id.is_none() {
        if let Some(identity) = identity {
            let candidates = conn.prepare("SELECT id,path FROM files WHERE volume_id=?1 AND file_identity=?2 AND trashed_at IS NULL")?
                .query_map(params![volume_id,identity], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
            if let [(candidate_id, candidate_path)] = candidates.as_slice() {
                let root: String = conn.query_row(
                    "SELECT path FROM volumes WHERE id=?1",
                    [&volume_id],
                    |row| row.get(0),
                )?;
                if candidate_path == &path {
                    id = Some(candidate_id.clone());
                } else {
                    match std::fs::symlink_metadata(Path::new(&root).join(candidate_path)) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            id = Some(candidate_id.clone())
                        }
                        Err(error) => return Err(sql_error(error)),
                        Ok(_) => {}
                    }
                }
            }
        }
    }
    let name = location
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let mime = (kind != Some(FileKind::Directory)).then(|| {
        let mime = plain_libs::utils::mime::mime_from_ext(name);
        if mime == "video/mp2t" && kind == Some(FileKind::File) {
            "application/octet-stream"
        } else {
            mime
        }
    });
    if let Some(id) = id {
        let source: String = conn.query_row(
            "SELECT path FROM file_catalog WHERE file_id=?1",
            [&id],
            |row| row.get(0),
        )?;
        let destination = path_text(&location.path)?;
        super::tag::move_file_path(conn, &source, &destination)?;
        if source != destination {
            conn.execute(
                "UPDATE image_embeddings SET path=?1 WHERE file_id=?2",
                params![destination, id],
            )?;
        }
        conn.execute("UPDATE files SET path=?1,file_name=?2,kind=COALESCE(?3,kind),file_identity=COALESCE(?4,file_identity),size_bytes=COALESCE(?5,size_bytes),created_at=COALESCE(created_at,?6),updated_at=?7,missing=0,indexed=1,mime_type=?8,last_seen_scan_id=COALESCE(?10,last_seen_scan_id) WHERE id=?9",
            params![path,name,kind,identity,size,created,updated,mime,id,scan_id])?;
        Ok(id)
    } else {
        let id = crate::features::record_id::generate();
        conn.execute("INSERT INTO files(id,volume_id,path,file_name,kind,file_identity,size_bytes,created_at,updated_at,mime_type,last_seen_scan_id) VALUES(?1,?2,?3,?4,COALESCE(?5,'FILE'),?6,?7,?8,?9,?10,?11)",
            params![id,volume_id,path,name,kind,identity,size,created,updated,mime,scan_id])?;
        Ok(id)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PathChange {
    Move,
    Trash,
    Restore,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PendingPathChange {
    source: String,
    destination: String,
    volume_identity: String,
    identity: Option<String>,
    change: PathChange,
}

pub(crate) fn recover_path_changes(conn: &Connection) -> plain_libs::rusqlite::Result<()> {
    let entries=conn.prepare("SELECT key,value FROM key_values WHERE substr(key,1,17)='file_path_change:' ORDER BY key")?
        .query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?
        .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
    for (key, value) in entries {
        let pending: PendingPathChange = plain_libs::serde_json::from_str(&value).map_err(sql_error)?;
        let source = Path::new(&pending.source);
        let destination = Path::new(&pending.destination);
        if !source.is_absolute() || !destination.is_absolute() {
            return Err(plain_libs::rusqlite::Error::InvalidQuery);
        }
        let source_volume = locate(&pending.source)?.volume.identity;
        let destination_volume = locate(&pending.destination)?.volume.identity;
        match plain_libs::utils::file_identity::within_volume_entry_key(source) {
            Ok(Some(identity)) if Some(identity.as_str()) == pending.identity.as_deref() => {
                if source_volume != pending.volume_identity {
                    return Err(plain_libs::rusqlite::Error::InvalidQuery);
                }
                conn.execute("DELETE FROM key_values WHERE key=?1", [key])?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match plain_libs::utils::file_identity::within_volume_entry_key(destination) {
                    Ok(Some(identity))
                        if Some(identity.as_str()) == pending.identity.as_deref() =>
                    {
                        if destination_volume != pending.volume_identity {
                            return Err(plain_libs::rusqlite::Error::InvalidQuery);
                        }
                        let tx = conn.unchecked_transaction()?;
                        change_paths(&tx, &pending.source, &pending.destination, pending.change)?;
                        tx.execute("DELETE FROM key_values WHERE key=?1", [key])?;
                        tx.commit()?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(sql_error(error)),
                    _ => return Err(plain_libs::rusqlite::Error::InvalidQuery),
                }
            }
            Err(error) => return Err(sql_error(error)),
            _ => return Err(plain_libs::rusqlite::Error::InvalidQuery),
        }
    }
    Ok(())
}

pub(crate) fn change_paths(
    conn: &Connection,
    source: &str,
    destination: &str,
    change: PathChange,
) -> plain_libs::rusqlite::Result<()> {
    let prefix = format!("{}/", source.trim_end_matches('/'));
    let rows=conn.prepare("SELECT file_id,path FROM file_catalog WHERE missing=0 AND (path=?1 OR substr(path,1,length(?2))=?2) ORDER BY file_id")?
        .query_map(params![source,prefix],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?
        .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
    let now = plain_libs::utils::dbtime::now_iso_millis();
    for (id, absolute) in rows {
        let suffix = Path::new(&absolute)
            .strip_prefix(source)
            .map_err(sql_error)?;
        let target = if suffix.as_os_str().is_empty() {
            Path::new(destination).to_owned()
        } else {
            Path::new(destination).join(suffix)
        };
        let location = locate(target.to_str().ok_or(plain_libs::rusqlite::Error::InvalidQuery)?)?;
        let (volume_id, path) = bind_volume(conn, &location)?;
        let identity = if std::fs::symlink_metadata(&target)
            .map_err(sql_error)?
            .file_type()
            .is_symlink()
        {
            None
        } else {
            plain_libs::utils::file_identity::within_volume_key(&target).map_err(sql_error)?
        };
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(plain_libs::rusqlite::Error::InvalidQuery)?;
        super::tag::move_file_path(conn, &absolute, &path_text(&location.path)?)?;
        conn.execute(
            "UPDATE files SET missing=1 WHERE volume_id=?1 AND path=?2 AND id<>?3 AND missing=0",
            params![volume_id, path, id],
        )?;
        conn.execute("UPDATE files SET original_path=CASE WHEN ?1='TRASH' THEN path WHEN ?1='RESTORE' THEN NULL ELSE original_path END,trashed_at=CASE WHEN ?1='TRASH' THEN ?2 WHEN ?1='RESTORE' THEN NULL ELSE trashed_at END,volume_id=?3,path=?4,file_name=CASE WHEN ?1='TRASH' THEN file_name ELSE ?5 END,file_identity=?6,missing=0 WHERE id=?7",
            params![match change { PathChange::Move=>"MOVE",PathChange::Trash=>"TRASH",PathChange::Restore=>"RESTORE" },now,volume_id,path,name,identity,id])?;
        conn.execute(
            "UPDATE image_embeddings SET path=?1 WHERE file_id=?2",
            params![target.to_str(), id],
        )?;
    }
    Ok(())
}

impl Db {
    pub fn mark_file_paths_missing(
        &self,
        paths: &[String],
    ) -> crate::infrastructure::error::DomainResult<()> {
        let paths = plain_libs::serde_json::to_string(paths)
            .map_err(|error| crate::infrastructure::error::DomainError::Other(error.to_string()))?;
        self.audio_transaction(|tx,state| {
            if let Some(id)=state.current_file_id.as_deref() {
                let affected=tx.query_row("SELECT EXISTS(SELECT 1 FROM file_catalog WHERE id=?1 AND path IN (SELECT value FROM json_each(?2)))",params![id,paths],|row|row.get::<_,bool>(0))?;
                if affected { state.select("")?; }
            }
            tx.execute("UPDATE files SET missing=1 WHERE id IN (SELECT file_id FROM file_catalog WHERE path IN (SELECT value FROM json_each(?1)))",[paths])?;
            Ok(())
        })
    }

    pub fn update_index_visibility(
        &self,
        scope: &crate::features::files::index_scope::Scope,
        paths: Option<&[String]>,
    ) -> plain_libs::rusqlite::Result<()> {
        self.with_conn(|connection| {
            let read_row = |row: &plain_libs::rusqlite::Row<'_>| -> plain_libs::rusqlite::Result<_> {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?, row.get::<_, FileKind>(3)?))
            };
            let changes = if let Some(paths) = paths {
                if paths.is_empty() { return Ok(()); }
                let volumes = connection.prepare("SELECT id,path FROM volumes WHERE instr(path,'://')=0")?
                    .query_map([], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?
                    .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
                let fields = "id,COALESCE(original_path,path),indexed,kind";
                let mut subtree = connection.prepare(&format!("SELECT {fields} FROM files WHERE volume_id=?1 AND path=?2 UNION ALL SELECT {fields} FROM files WHERE volume_id=?1 AND path>=?3 AND path<?4"))?;
                let mut volume_rows = connection.prepare(&format!("SELECT {fields} FROM files WHERE volume_id=?1"))?;
                let mut changes = std::collections::BTreeMap::new();
                for path in paths {
                    for (volume_id, volume_path) in &volumes {
                        let root = volume_path.trim_end_matches('/');
                        let prefix = format!("{root}/");
                        let relative = if path == root { Some("") } else { path.strip_prefix(&prefix) };
                        let rows = if relative == Some("") || root.strip_prefix(path).is_some_and(|suffix| suffix.starts_with('/')) || (path == "/" && root.starts_with('/')) {
                            volume_rows.query_map([volume_id], read_row)?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()?
                        } else if let Some(relative) = relative {
                            subtree.query_map(params![volume_id,relative,format!("{relative}/"),format!("{relative}0")], read_row)?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()?
                        } else { continue; };
                        for (id, relative, indexed, kind) in rows {
                            let absolute = if relative.is_empty() { volume_path.clone() } else { format!("{root}/{relative}") };
                            changes.insert(id, (absolute,indexed,kind));
                        }
                    }
                }
                changes.into_iter().map(|(id,(path,indexed,kind))|(id,path,indexed,kind)).collect::<Vec<_>>()
            } else {
                connection.prepare("SELECT file_id,COALESCE(original_path,path),indexed,kind FROM file_catalog WHERE instr(path,'://')=0")?
                    .query_map([],read_row)?.collect::<plain_libs::rusqlite::Result<Vec<_>>>()?
            };
            let tx = connection.unchecked_transaction()?;
            for (id, path, indexed, kind) in changes {
                let visible = scope.contains_entry(Path::new(&path), kind == FileKind::Directory);
                if visible != indexed { tx.execute("UPDATE files SET indexed=?1 WHERE id=?2", params![visible,id])?; }
            }
            tx.commit()
        })
    }

    pub fn refresh_volume_availability(&self) -> plain_libs::rusqlite::Result<()> {
        self.with_conn(|conn| {
            let volumes = conn
                .prepare("SELECT id,identity,path FROM volumes WHERE instr(path,'://')=0")?
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        Volume {
                            identity: row.get(1)?,
                            path: row.get::<_, String>(2)?.into(),
                        },
                    ))
                })?
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
            let facts = volumes
                .iter()
                .map(|(id, volume)| Ok((id, root_is_current(conn, id, volume)?)))
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
            let tx = conn.unchecked_transaction()?;
            for (id, available) in facts {
                tx.execute(
                    "UPDATE volumes SET is_available=?1 WHERE id=?2 AND is_available<>?1",
                    params![available, id],
                )?;
            }
            tx.commit()
        })
    }
    pub fn prepare_file_path_change(
        &self,
        id: &str,
        source: &str,
        destination: &str,
        change: PathChange,
    ) -> plain_libs::rusqlite::Result<()> {
        let identity = plain_libs::utils::file_identity::within_volume_entry_key(Path::new(source))
            .map_err(sql_error)?;
        let volume_identity = locate(source)?.volume.identity;
        if locate(destination)?.volume.identity != volume_identity {
            return Err(plain_libs::rusqlite::Error::InvalidQuery);
        }
        let pending = PendingPathChange {
            source: source.into(),
            destination: destination.into(),
            volume_identity,
            identity,
            change,
        };
        let value = plain_libs::serde_json::to_string(&pending).map_err(sql_error)?;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO key_values(key,value) VALUES(?1,?2)",
                params![format!("file_path_change:{id}"), value],
            )
            .map(|_| ())
        })
    }

    pub fn finish_file_path_change(
        &self,
        id: &str,
        source: &str,
        destination: &str,
        change: PathChange,
    ) -> plain_libs::rusqlite::Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            change_paths(&tx, source, destination, change)?;
            tx.execute(
                "DELETE FROM key_values WHERE key=?1",
                [format!("file_path_change:{id}")],
            )?;
            tx.commit()
        })
    }
    pub fn change_file_paths(
        &self,
        source: &str,
        destination: &str,
        change: PathChange,
    ) -> plain_libs::rusqlite::Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            change_paths(&tx, source, destination, change)?;
            tx.commit()
        })
    }
    pub fn register_volume(&self, path: &Path) -> plain_libs::rusqlite::Result<Option<String>> {
        let Some(location) = available_location(path)? else {
            return Ok(None);
        };
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let (id, _) = bind_volume(&tx, &location)?;
            tx.commit()?;
            Ok(Some(id))
        })
    }

    pub fn set_volume_index(
        &self,
        id: &str,
        mode: crate::infrastructure::db::IndexMode,
        included_dirs: &[String],
        excluded_dirs: &[String],
    ) -> plain_libs::rusqlite::Result<bool> {
        let included = plain_libs::serde_json::to_string(included_dirs).map_err(sql_error)?;
        let excluded = plain_libs::serde_json::to_string(excluded_dirs).map_err(sql_error)?;
        self.with_conn(|conn| {
            Ok(conn.execute(
                "UPDATE volumes SET index_mode=?1,included_dirs=?2,excluded_dirs=?3 WHERE id=?4",
                params![mode, included, excluded, id],
            )? != 0)
        })
    }

    pub fn volumes(&self) -> plain_libs::rusqlite::Result<Vec<VolumeRow>> {
        self.with_conn(|conn| {
            conn.prepare("SELECT id,identity,path,is_available,index_mode,included_dirs,excluded_dirs FROM volumes ORDER BY path")?
                .query_map([], |row| {
                    let path: String = row.get(2)?;
                    let path = if path.contains("://") { path } else { crate::features::files::index_scope::normalized(Path::new(&path)) };
                    Ok(VolumeRow {
                        id: row.get(0)?,
                        identity: row.get(1)?,
                        path,
                        is_available: row.get(3)?,
                        index_mode: row.get(4)?,
                        included_dirs: plain_libs::serde_json::from_str(&row.get::<_, String>(5)?).map_err(sql_error)?,
                        excluded_dirs: plain_libs::serde_json::from_str(&row.get::<_, String>(6)?).map_err(sql_error)?,
                    })
                })?
                .collect()
        })
    }

    pub fn file(&self, id: &str) -> plain_libs::rusqlite::Result<Option<FileRow>> {
        self.with_conn(|conn| {
            conn.query_row(
                &format!("SELECT {COLUMNS} FROM files WHERE id=?1"),
                [id],
                from_row,
            )
            .optional()
        })
    }

    pub fn record_file_paths(
        &self,
        paths: &[String],
        file_kind: Option<FileKind>,
    ) -> plain_libs::rusqlite::Result<Vec<FileRow>> {
        self.record_file_path_inputs(
            paths.iter().map(|path| (path.as_str(), file_kind)),
            None,
            |conn, id| {
                conn.query_row(
                    &format!("SELECT {COLUMNS} FROM files WHERE id=?1"),
                    [id],
                    from_row,
                )
            },
        )
    }

    pub(crate) fn record_scan_paths(
        &self,
        paths: &[(String, FileKind)],
        scan_id: &str,
    ) -> plain_libs::rusqlite::Result<()> {
        self.record_file_path_inputs(
            paths
                .iter()
                .map(|(path, kind)| (path.as_str(), Some(*kind))),
            Some(scan_id),
            |_, _| Ok(()),
        )
        .map(|_| ())
    }

    fn record_file_path_inputs<'a, T>(
        &self,
        paths: impl Iterator<Item = (&'a str, Option<FileKind>)>,
        scan_id: Option<&str>,
        read: impl Fn(&Connection, String) -> plain_libs::rusqlite::Result<T>,
    ) -> plain_libs::rusqlite::Result<Vec<T>> {
        let facts = paths
            .map(|(path, file_kind)| {
                let location = locate(path)?;
                let metadata = std::fs::symlink_metadata(&location.path).map_err(sql_error)?;
                let identity = if metadata.file_type().is_symlink() {
                    None
                } else {
                    plain_libs::utils::file_identity::within_volume_key(&location.path)
                        .map_err(sql_error)?
                };
                let kind = if metadata.is_dir() {
                    Some(FileKind::Directory)
                } else {
                    file_kind
                };
                let created = metadata.created().ok().map(|time| {
                    plain_libs::chrono::DateTime::<plain_libs::chrono::Utc>::from(time)
                        .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
                });
                let updated =
                    plain_libs::chrono::DateTime::<plain_libs::chrono::Utc>::from(metadata.modified().map_err(sql_error)?)
                        .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true);
                let size = i64::try_from(metadata.len()).map_err(sql_error)?;
                Ok((location, kind, identity, size, created, updated))
            })
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let rows = facts
                .iter()
                .map(|(location, kind, identity, size, created, updated)| {
                    let id = upsert(
                        &tx,
                        location,
                        *kind,
                        identity.as_deref(),
                        if *kind == Some(FileKind::Directory) {
                            None
                        } else {
                            Some(*size)
                        },
                        created.as_deref(),
                        updated,
                        scan_id,
                    )?;
                    read(&tx, id)
                })
                .collect::<plain_libs::rusqlite::Result<Vec<_>>>()?;
            tx.commit()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/infrastructure/db/repositories/files.rs"]
mod tests;
