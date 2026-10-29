use crate::db::{Db, FileKind, FileRow, VolumeRow};
use crate::platform::volumes::{self, Location, Volume};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Component, Path};

pub const COLUMNS: &str = "id,volume_id,path,file_name,kind,file_identity,mime_type,size_bytes,created_at,updated_at,missing,favorited,trashed_at,original_path,duration_ms,width,height,rotation,taken_at,title,artist,album,album_artist,album_id,cover_image_id";

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
                "file_id IN (SELECT rowid FROM file_names_fts WHERE file_names_fts MATCH ?) AND {exact}"
            ),
            vec![format!("\"{}\"", text.replace('"', "\"\"")), literal],
        )
    }
}

pub fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRow> {
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
    })
}

pub(crate) fn sql_error(
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(error.into())
}

pub fn relative_path(path: &Path) -> rusqlite::Result<String> {
    path.components()
        .map(|part| match part {
            Component::Normal(value) => value
                .to_str()
                .filter(|value| !value.contains(['\\', '\0']))
                .map(str::to_owned)
                .ok_or(rusqlite::Error::InvalidQuery),
            _ => Err(rusqlite::Error::InvalidQuery),
        })
        .collect::<rusqlite::Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

fn path_text(path: &Path) -> rusqlite::Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(rusqlite::Error::InvalidQuery)
}

pub(crate) fn locate(path: &str) -> rusqlite::Result<Location> {
    volumes::locate(Path::new(path)).map_err(|error| sql_error(error.into_boxed_dyn_error()))
}

fn root_is_current(conn: &Connection, id: i64, old: &Volume) -> rusqlite::Result<bool> {
    let matches = |path: &Path| {
        volumes::locate(path)
            .map(|location| location.volume == *old)
            .map_err(|error| sql_error(error.into_boxed_dyn_error()))
    };
    if matches(&old.path)? {
        return Ok(true);
    }
    let mut paths = conn.prepare("SELECT path FROM files WHERE volume_id=?1 ORDER BY id")?;
    for path in paths.query_map([id], |row| row.get::<_, String>(0))? {
        if matches(&old.path.join(path?))? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn bind_volume(
    conn: &Connection,
    location: &Location,
) -> rusqlite::Result<(i64, String)> {
    let existing: Option<(i64, String)> = conn
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
        } else if root_is_current(conn, id, &old)? && location.path.starts_with(&old.path) {
            old.path
        } else if old.path.starts_with(candidate) && root_is_current(conn, id, &old)? {
            let prefix = relative_path(old.path.strip_prefix(candidate).map_err(sql_error)?)?;
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
        conn.execute(
            "INSERT INTO volumes(identity,path,is_available) VALUES(?1,?2,1)",
            params![location.volume.identity, path_text(candidate)?],
        )?;
        (conn.last_insert_rowid(), candidate.clone())
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
) -> rusqlite::Result<i64> {
    let (volume_id, path) = bind_volume(conn, location)?;
    let existing: Option<(i64, Option<String>)> = conn
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
                .query_map(params![volume_id,identity], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if let [(candidate_id, candidate_path)] = candidates.as_slice() {
                let root: String =
                    conn.query_row("SELECT path FROM volumes WHERE id=?1", [volume_id], |row| {
                        row.get(0)
                    })?;
                if candidate_path == &path {
                    id = Some(*candidate_id);
                } else {
                    match std::fs::symlink_metadata(Path::new(&root).join(candidate_path)) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            id = Some(*candidate_id)
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
    let mime = (kind != Some(FileKind::Directory)).then(|| crate::utils::mime::mime_from_ext(name));
    if let Some(id) = id {
        let source: String = conn.query_row(
            "SELECT path FROM file_catalog WHERE file_id=?1",
            [id],
            |row| row.get(0),
        )?;
        let destination = path_text(&location.path)?;
        super::tag::move_file_path(conn, &source, &destination)?;
        if source != destination {
            conn.execute(
                "UPDATE image_embeddings SET path=?1 WHERE file_id=?2",
                params![destination, id.to_string()],
            )?;
        }
        conn.execute("UPDATE files SET path=?1,file_name=?2,kind=COALESCE(?3,kind),file_identity=COALESCE(?4,file_identity),size_bytes=COALESCE(?5,size_bytes),created_at=COALESCE(created_at,?6),updated_at=?7,missing=0,mime_type=?8 WHERE id=?9",
            params![path,name,kind,identity,size,created,updated,mime,id])?;
        Ok(id)
    } else {
        conn.execute("INSERT INTO files(volume_id,path,file_name,kind,file_identity,size_bytes,created_at,updated_at,mime_type) VALUES(?1,?2,?3,COALESCE(?4,'FILE'),?5,?6,?7,?8,?9)",
            params![volume_id,path,name,kind,identity,size,created,updated,mime])?;
        Ok(conn.last_insert_rowid())
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

pub(crate) fn recover_path_changes(conn: &Connection) -> rusqlite::Result<()> {
    let entries=conn.prepare("SELECT key,value FROM key_values WHERE substr(key,1,17)='file_path_change:' ORDER BY key")?
        .query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (key, value) in entries {
        let pending: PendingPathChange = serde_json::from_str(&value).map_err(sql_error)?;
        let source = Path::new(&pending.source);
        let destination = Path::new(&pending.destination);
        if !source.is_absolute() || !destination.is_absolute() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let source_volume = locate(&pending.source)?.volume.identity;
        let destination_volume = locate(&pending.destination)?.volume.identity;
        match crate::utils::file_identity::within_volume_entry_key(source) {
            Ok(Some(identity)) if Some(identity.as_str()) == pending.identity.as_deref() => {
                if source_volume != pending.volume_identity {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                conn.execute("DELETE FROM key_values WHERE key=?1", [key])?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match crate::utils::file_identity::within_volume_entry_key(destination) {
                    Ok(Some(identity))
                        if Some(identity.as_str()) == pending.identity.as_deref() =>
                    {
                        if destination_volume != pending.volume_identity {
                            return Err(rusqlite::Error::InvalidQuery);
                        }
                        let tx = conn.unchecked_transaction()?;
                        change_paths(&tx, &pending.source, &pending.destination, pending.change)?;
                        tx.execute("DELETE FROM key_values WHERE key=?1", [key])?;
                        tx.commit()?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(sql_error(error)),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                }
            }
            Err(error) => return Err(sql_error(error)),
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
    }
    Ok(())
}

pub(crate) fn change_paths(
    conn: &Connection,
    source: &str,
    destination: &str,
    change: PathChange,
) -> rusqlite::Result<()> {
    let prefix = format!("{}/", source.trim_end_matches('/'));
    let rows=conn.prepare("SELECT file_id,path FROM file_catalog WHERE missing=0 AND (path=?1 OR substr(path,1,length(?2))=?2) ORDER BY file_id")?
        .query_map(params![source,prefix],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let now = crate::utils::dbtime::now_iso_millis();
    for (id, absolute) in rows {
        let suffix = Path::new(&absolute)
            .strip_prefix(source)
            .map_err(sql_error)?;
        let target = if suffix.as_os_str().is_empty() {
            Path::new(destination).to_owned()
        } else {
            Path::new(destination).join(suffix)
        };
        let location = locate(target.to_str().ok_or(rusqlite::Error::InvalidQuery)?)?;
        let (volume_id, path) = bind_volume(conn, &location)?;
        let identity = if std::fs::symlink_metadata(&target)
            .map_err(sql_error)?
            .file_type()
            .is_symlink()
        {
            None
        } else {
            crate::utils::file_identity::within_volume_key(&target).map_err(sql_error)?
        };
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(rusqlite::Error::InvalidQuery)?;
        super::tag::move_file_path(conn, &absolute, &path_text(&location.path)?)?;
        conn.execute(
            "UPDATE files SET missing=1 WHERE volume_id=?1 AND path=?2 AND id<>?3 AND missing=0",
            params![volume_id, path, id],
        )?;
        conn.execute("UPDATE files SET original_path=CASE WHEN ?1='TRASH' THEN path WHEN ?1='RESTORE' THEN NULL ELSE original_path END,trashed_at=CASE WHEN ?1='TRASH' THEN ?2 WHEN ?1='RESTORE' THEN NULL ELSE trashed_at END,volume_id=?3,path=?4,file_name=CASE WHEN ?1='TRASH' THEN file_name ELSE ?5 END,file_identity=?6,missing=0 WHERE id=?7",
            params![match change { PathChange::Move=>"MOVE",PathChange::Trash=>"TRASH",PathChange::Restore=>"RESTORE" },now,volume_id,path,name,identity,id])?;
        conn.execute(
            "UPDATE image_embeddings SET path=?1 WHERE file_id=?2",
            params![target.to_str(), id.to_string()],
        )?;
    }
    Ok(())
}

impl Db {
    pub fn mark_file_paths_missing(&self, paths: &[String]) -> crate::error::DomainResult<()> {
        let paths = serde_json::to_string(paths)
            .map_err(|error| crate::error::DomainError::Other(error.to_string()))?;
        self.audio_transaction(|tx,state| {
            if let Some(id)=state.current_file_id.as_deref() {
                let affected=tx.query_row("SELECT EXISTS(SELECT 1 FROM file_catalog WHERE id=?1 AND path IN (SELECT value FROM json_each(?2)))",params![id,paths],|row|row.get::<_,bool>(0))?;
                if affected { state.select("")?; }
            }
            tx.execute("UPDATE files SET missing=1 WHERE id IN (SELECT file_id FROM file_catalog WHERE path IN (SELECT value FROM json_each(?1)))",[paths])?;
            Ok(())
        })
    }

    pub fn refresh_volume_availability(&self) -> rusqlite::Result<()> {
        self.with_conn(|conn| {
            let volumes = conn
                .prepare("SELECT id,identity,path FROM volumes")?
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        Volume {
                            identity: row.get(1)?,
                            path: row.get::<_, String>(2)?.into(),
                        },
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let facts = volumes
                .iter()
                .map(|(id, volume)| Ok((*id, root_is_current(conn, *id, volume)?)))
                .collect::<rusqlite::Result<Vec<_>>>()?;
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
        id: i64,
        source: &str,
        destination: &str,
        change: PathChange,
    ) -> rusqlite::Result<()> {
        let identity = crate::utils::file_identity::within_volume_entry_key(Path::new(source))
            .map_err(sql_error)?;
        let volume_identity = locate(source)?.volume.identity;
        if locate(destination)?.volume.identity != volume_identity {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let pending = PendingPathChange {
            source: source.into(),
            destination: destination.into(),
            volume_identity,
            identity,
            change,
        };
        let value = serde_json::to_string(&pending).map_err(sql_error)?;
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
        id: i64,
        source: &str,
        destination: &str,
        change: PathChange,
    ) -> rusqlite::Result<()> {
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
    ) -> rusqlite::Result<()> {
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            change_paths(&tx, source, destination, change)?;
            tx.commit()
        })
    }
    pub fn volumes(&self) -> rusqlite::Result<Vec<VolumeRow>> {
        self.with_conn(|conn| {
            conn.prepare("SELECT id,identity,path,is_available FROM volumes ORDER BY id")?
                .query_map([], |row| {
                    Ok(VolumeRow {
                        id: row.get(0)?,
                        identity: row.get(1)?,
                        path: row.get(2)?,
                        is_available: row.get(3)?,
                    })
                })?
                .collect()
        })
    }

    pub fn file(&self, id: i64) -> rusqlite::Result<Option<FileRow>> {
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
    ) -> rusqlite::Result<Vec<FileRow>> {
        let facts = paths
            .iter()
            .map(|path| {
                let location = locate(path)?;
                let metadata = std::fs::symlink_metadata(&location.path).map_err(sql_error)?;
                let identity = if metadata.file_type().is_symlink() {
                    None
                } else {
                    crate::utils::file_identity::within_volume_key(&location.path)
                        .map_err(sql_error)?
                };
                let kind = if metadata.is_dir() {
                    Some(FileKind::Directory)
                } else {
                    file_kind
                };
                let created = metadata.created().ok().map(|time| {
                    chrono::DateTime::<chrono::Utc>::from(time)
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                });
                let updated =
                    chrono::DateTime::<chrono::Utc>::from(metadata.modified().map_err(sql_error)?)
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                let size = i64::try_from(metadata.len()).map_err(sql_error)?;
                Ok((location, kind, identity, size, created, updated))
            })
            .collect::<rusqlite::Result<Vec<_>>>()?;
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
                    )?;
                    tx.query_row(
                        &format!("SELECT {COLUMNS} FROM files WHERE id=?1"),
                        [id],
                        from_row,
                    )
                })
                .collect::<rusqlite::Result<Vec<_>>>()?;
            tx.commit()?;
            Ok(rows)
        })
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/db/repositories/files.rs"]
mod tests;
