use crate::features::chat::attachments::app_file_store;
use crate::infrastructure::data_paths::DataPaths;
use plain_libs::anyhow::{Context, Result, ensure};
use plain_libs::rusqlite::{Connection, OptionalExtension, params};
use plain_libs::temporary::NamedTempFile;
use plain_libs::utils::{dbtime, mime};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(super) struct Assets<'a> {
    source: &'a Path,
    target: &'a Path,
    db: &'a Connection,
    desktop_db: Option<&'a Connection>,
    file_names: HashMap<String, String>,
    old_paths: BTreeSet<PathBuf>,
    installed: InstalledAssets,
}

impl<'a> Assets<'a> {
    pub fn new(source: &'a Path, target: &'a Path, db: &'a Connection) -> Self {
        Self {
            source,
            target,
            db,
            desktop_db: None,
            file_names: HashMap::new(),
            old_paths: BTreeSet::new(),
            installed: InstalledAssets::default(),
        }
    }

    pub fn with_desktop_db(mut self, db: &'a Connection) -> Self {
        self.desktop_db = Some(db);
        self
    }

    pub fn finish(self) -> (Vec<PathBuf>, InstalledAssets) {
        (self.old_paths.into_iter().collect(), self.installed)
    }

    pub fn app_file(
        &mut self,
        id: String,
        size: i64,
        mime: String,
        path: String,
        count: i64,
        created: String,
        updated: String,
    ) -> Result<()> {
        ensure!(
            id.len() == 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "Invalid legacy app file ID"
        );
        let source = source_path(self.source, Path::new(&path))?;
        let (weak, actual_size) = app_file_store::weak_hash_file(&source)?;
        ensure!(
            i64::try_from(actual_size)? == size,
            "Legacy app file size differs from its record"
        );
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("Invalid legacy app file name")?;
        ensure!(
            name.split('.').next() == Some(id.as_str())
                && source
                    .parent()
                    .is_some_and(|parent| parent.ends_with(Path::new(&id[..2]).join(&id[2..4]))),
            "Legacy app file is outside its shard"
        );
        let mime = if mime.is_empty() {
            "application/octet-stream"
        } else {
            &mime
        };
        let suffix = self.install_hashed(
            &source, name, &id, &weak, size, mime, count, &created, &updated,
        )?;
        self.file_names.insert(id, suffix);
        Ok(())
    }

    pub fn fid(&self, value: &str) -> Result<String> {
        let suffix = value
            .strip_prefix("fid:")
            .context("Invalid legacy app file reference")?;
        let id = suffix.split('.').next().unwrap_or_default();
        let name = self
            .file_names
            .get(id)
            .context("Legacy app file record is missing")?;
        Ok(format!("fid:{name}"))
    }

    pub fn chat_uri(&mut self, value: &str, created: &str, updated: &str) -> Result<String> {
        if let Some(suffix) = value.strip_prefix("fid:") {
            let id = suffix.split('.').next().unwrap_or_default();
            if let Some(source) = self.desktop_db
                && !self.file_names.contains_key(id)
            {
                let (size, mime, path, count, created, updated) = source.query_row(
                    "SELECT size, mime_type, real_path, ref_count, created_at, updated_at FROM app_files WHERE id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                ).context("Legacy desktop attachment record is missing")?;
                self.app_file(id.into(), size, mime, path, count, created, updated)?;
            }
            return self.fid(value);
        }
        let relative = value
            .strip_prefix("app://")
            .map(Path::new)
            .or_else(|| Path::new(value).strip_prefix(self.source).ok());
        match relative {
            Some(path) => Ok(format!("fid:{}", self.store_file(path, created, updated)?)),
            None => Ok(value.to_owned()),
        }
    }

    pub fn note_images(&mut self, text: &str, created: &str, updated: &str) -> Result<String> {
        let mut result = String::with_capacity(text.len());
        let mut remaining = text;
        while let Some((prefix, suffix)) = remaining.split_once("app://note-images/") {
            result.push_str(prefix);
            let end = suffix
                .find([')', '>', '\'', '"', '\n', '\r'])
                .unwrap_or(suffix.len());
            let name = &suffix[..end];
            let fid = self.note_image(name, created, updated)?;
            result.push_str("fid:");
            result.push_str(&fid);
            remaining = &suffix[end..];
        }
        result.push_str(remaining);
        Ok(result)
    }

    fn note_image(&mut self, name: &str, created: &str, updated: &str) -> Result<String> {
        ensure!(
            matches!(
                Path::new(name).components().collect::<Vec<_>>().as_slice(),
                [Component::Normal(_)]
            ),
            "Invalid legacy note image name"
        );
        self.store_file(&Path::new("note-images").join(name), created, updated)
    }

    fn store_file(&mut self, relative: &Path, created: &str, updated: &str) -> Result<String> {
        let source = source_path(self.source, relative)?;
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("Invalid legacy file name")?;
        let strong = app_file_store::strong_hash_file(&source)?;
        let (weak, size) = app_file_store::weak_hash_file(&source)?;
        self.install_hashed(
            &source,
            name,
            &strong,
            &weak,
            i64::try_from(size)?,
            mime::mime_from_ext(name),
            1,
            created,
            updated,
        )
    }

    pub fn pomodoro_sound(&mut self) -> Result<()> {
        let relative = Path::new("audio/pomodoro_sound.mp3");
        let source = source_path(self.source, relative)?;
        let path = DataPaths::new(self.target).pomodoro_sound();
        self.install(&source, &path, None)
    }

    fn install_hashed(
        &mut self,
        source: &Path,
        name: &str,
        id: &str,
        weak: &str,
        size: i64,
        mime: &str,
        count: i64,
        created: &str,
        updated: &str,
    ) -> Result<String> {
        let existing: Option<String> = self
            .db
            .query_row("SELECT real_path FROM app_files WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        let stored = existing.unwrap_or_else(|| {
            app_file_store::relative_dest_path(id, &app_file_store::ext_from_name(name, mime))
        });
        let relative = Path::new(&stored);
        ensure!(
            relative.starts_with("files") && clean_relative(relative),
            "Invalid current app file path"
        );
        self.install(source, &self.target.join(relative), Some(id))?;
        self.db.execute(
            "INSERT INTO app_files(id,size_bytes,mime_type,real_path,ref_count,weak_hash,created_at,updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET ref_count=ref_count+excluded.ref_count",
            params![id, size, mime, stored, count, weak, dbtime::normalize(created)?, dbtime::normalize(updated)?],
        )?;
        Ok(relative
            .file_name()
            .and_then(|name| name.to_str())
            .context("Invalid app file name")?
            .into())
    }

    fn install(&mut self, source: &Path, destination: &Path, expected: Option<&str>) -> Result<()> {
        let root = self.target.canonicalize()?;
        let parent = destination
            .parent()
            .context("Missing migration asset directory")?;
        fs::create_dir_all(parent)?;
        ensure!(
            parent.canonicalize()?.starts_with(&root),
            "App file destination escapes the data directory"
        );
        if destination.try_exists()? {
            ensure!(
                !destination.symlink_metadata()?.is_symlink(),
                "Invalid migration asset destination"
            );
            let destination = destination.canonicalize()?;
            if destination == source {
                return Ok(());
            }
            let expected = match expected {
                Some(value) => value.to_owned(),
                None => app_file_store::strong_hash_file(source)?,
            };
            ensure!(
                app_file_store::strong_hash_file(&destination)? == expected,
                "Migration asset conflicts with a current file"
            );
            ensure!(
                app_file_store::strong_hash_file(source)? == expected,
                "Legacy app file checksum differs from its record"
            );
        } else {
            let mut staging = NamedTempFile::new_in(parent)?;
            std::io::copy(&mut fs::File::open(source)?, staging.as_file_mut())?;
            if let Some(expected) = expected {
                ensure!(
                    app_file_store::strong_hash_file(staging.path())? == expected,
                    "Legacy app file checksum differs from its record"
                );
            }
            staging.as_file().sync_all()?;
            staging.persist_noclobber(destination)?;
            self.installed.paths.push(destination.to_owned());
        }
        self.old_paths
            .insert(source.strip_prefix(self.source.canonicalize()?)?.to_owned());
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct InstalledAssets {
    paths: Vec<PathBuf>,
}

impl InstalledAssets {
    pub fn committed(mut self) {
        self.paths.clear();
    }
}

impl Drop for InstalledAssets {
    fn drop(&mut self) {
        for path in &self.paths {
            if let Err(error) = fs::remove_file(path) {
                log::warn!("Failed to remove an incomplete legacy import asset: {error}");
            }
        }
    }
}

pub(super) fn clean_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

pub(super) fn source_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let real_root = root.canonicalize()?;
    let real_source = if path.is_absolute() {
        path.canonicalize()?
    } else {
        ensure!(clean_relative(path), "Invalid legacy file path");
        real_root.join(path).canonicalize()?
    };
    ensure!(
        real_source.starts_with(real_root) && real_source.is_file(),
        "Legacy asset escapes its storage root"
    );
    Ok(real_source)
}
