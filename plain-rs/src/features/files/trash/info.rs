use super::{
    TrashItem, is_in_plain_trash, parse_trash_id_from_path, trash_root, validate_directory,
    validate_trash_root,
};
use crate::infrastructure::db::{Db, FileKind, files};
use plain_libs::anyhow::{Context, Result, ensure};
use plain_libs::chrono::{DateTime, Utc};
use plain_libs::platform::volumes::{self, Location};
use plain_libs::serde_json;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TrashInfo {
    pub(super) original_path: String,
    pub(super) trash_path: String,
    pub(super) trashed_at: String,
}

impl TrashInfo {
    pub(super) fn new(source: &Location, trash_path: String, when: DateTime<Utc>) -> Result<Self> {
        Ok(Self {
            original_path: files::relative_path(source.path.strip_prefix(&source.volume.path)?)?,
            trash_path,
            trashed_at: when.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true),
        })
    }

    pub(super) fn write(&self, disk: &str, id: &str) -> Result<()> {
        let path = info_path(disk, id)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let old = Self::read_record(&path)?;
                let payload = trash_root(disk).join(old.payload(id)?);
                match fs::symlink_metadata(payload) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                    Ok(_) => plain_libs::anyhow::bail!("A trash payload already uses this file ID"),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut temp = plain_libs::temporary::NamedTempFile::new_in(path.parent().unwrap())?;
        serde_json::to_writer(&mut temp, self)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(path)?;
        Ok(())
    }

    pub(super) fn read(item: &TrashItem) -> Result<Self> {
        let path = info_path(&item.disk, &item.id)?;
        let info = Self::read_record(&path)?;
        ensure!(
            trash_root(&item.disk).join(info.payload(&item.id)?) == item.path,
            "Trash restore record does not match its payload"
        );
        validate_payload_parents(item)?;
        fs::symlink_metadata(&item.path)?;
        Ok(info)
    }

    fn read_record(path: &Path) -> Result<Self> {
        let meta = fs::symlink_metadata(path).context("Trash restore record is unavailable")?;
        ensure!(
            meta.is_file() && meta.len() <= 65_536,
            "Invalid trash restore record"
        );
        let info: Self = serde_json::from_reader(fs::File::open(path)?)
            .context("Invalid trash restore record")?;
        DateTime::parse_from_rfc3339(&info.trashed_at).context("Invalid trash timestamp")?;
        Ok(info)
    }

    fn payload(&self, id: &str) -> Result<&Path> {
        let path = strict_relative(&self.trash_path)?;
        let mut components = path.components();
        ensure!(
            components
                .next()
                .is_some_and(|component| component.as_os_str() == "data")
                && components.count() == 3
                && parse_trash_id_from_path(path).as_deref() == Some(id),
            "Invalid trash restore payload path"
        );
        Ok(path)
    }

    pub(super) fn rebuild_index(&self, db: &Db, item: &TrashItem, target: &Location) -> Result<()> {
        use plain_libs::rusqlite::params;
        let source = volumes::locate(&item.path)?;
        let metadata = fs::symlink_metadata(&source.path)?;
        let kind = if metadata.is_dir() {
            FileKind::Directory
        } else {
            FileKind::File
        };
        let name = target
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .context("Restore name required")?;
        let size = (!metadata.is_dir()).then_some(i64::try_from(metadata.len())?);
        let mime = (!metadata.is_dir()).then(|| plain_libs::utils::mime::mime_from_ext(name));
        let identity = if metadata.file_type().is_symlink() {
            None
        } else {
            plain_libs::utils::file_identity::within_volume_key(&source.path)?
        };
        let updated = DateTime::<Utc>::from(metadata.modified()?)
            .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true);
        db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let (volume_id, path) = files::bind_volume(&tx, &source)?;
            let (target_volume, original_path) = files::bind_volume(&tx, target)?;
            if target_volume != volume_id {
                return Err(plain_libs::rusqlite::Error::InvalidQuery);
            }
            tx.execute(
                "INSERT INTO files(id,volume_id,path,file_name,kind,file_identity,size_bytes,updated_at,mime_type,original_path,trashed_at)
                VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![item.id, volume_id, path, name, kind, identity, size, updated, mime, original_path, self.trashed_at],
            )?;
            tx.commit()
        })?;
        Ok(())
    }

    pub(super) fn target(&self, item: &TrashItem) -> Result<Location> {
        let relative = strict_relative(&self.original_path)?;
        ensure!(
            !is_in_plain_trash(relative),
            "Restore target is inside trash"
        );
        let source = volumes::locate(&item.path)?;
        let expected = source.volume.path.join(relative);
        // Resolve the parent, not an existing file that may cause a name conflict.
        let parent = volumes::locate(expected.parent().context("Restore parent required")?)?;
        ensure!(
            source.path == item.path
                && parent.volume == source.volume
                && Some(parent.path.as_path()) == expected.parent(),
            "Restore path escapes the original volume or follows a symlink"
        );
        ensure!(
            Path::new(&item.disk) == source.volume.path
                || Some(Path::new(&item.disk)) == expected.parent(),
            "Trash folder does not belong to the restore path"
        );
        Ok(Location {
            volume: source.volume,
            path: expected,
        })
    }
}

pub(super) fn validate_payload_parents(item: &TrashItem) -> Result<()> {
    let root = trash_root(&item.disk);
    let parent = item
        .path
        .parent()
        .context("Trash payload parent required")?;
    ensure!(parent.starts_with(&root), "Invalid trash payload");
    for ancestor in parent.ancestors().take_while(|path| *path != root) {
        validate_directory(ancestor)?;
    }
    Ok(())
}

pub(super) fn remove(disk: &str, id: &str) -> Result<()> {
    match info_path(disk, id).and_then(|path| fs::remove_file(path).map_err(Into::into)) {
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
        {
            Ok(())
        }
        result => result,
    }
}

fn info_path(disk: &str, id: &str) -> Result<PathBuf> {
    ensure!(
        !id.is_empty() && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'),
        "Invalid trash file ID"
    );
    let root = trash_root(disk);
    validate_trash_root(&root)?;
    let info = root.join("info");
    validate_directory(&info)?;
    Ok(info.join(format!("{id}.json")))
}

fn strict_relative(value: &str) -> Result<&Path> {
    let path = Path::new(value);
    ensure!(
        !value.is_empty() && files::relative_path(path)? == value,
        "Invalid relative trash path"
    );
    Ok(path)
}
