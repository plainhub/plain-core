mod paths;
mod rules;

pub use paths::{
    absolute_directory, normalized, relative_directories, under, validate_directories,
};
pub use rules::{Defaults, defaults};
#[cfg(target_os = "macos")]
pub(crate) use rules::{
    macos_application_directory, macos_cloud_documents_ancestor, macos_cloud_documents_root,
};

use crate::infrastructure::db::{Db, FileIndexMode, VolumeRow};
use std::path::Path;

#[derive(Clone)]
pub struct Scope {
    pub directories: Vec<String>,
    pub exclusions: Vec<String>,
    pub defaults: Defaults,
    pub volumes: Vec<VolumeRow>,
}

impl Scope {
    pub fn load(
        db: &Db,
        data: &Path,
        cache: &Path,
        extra: &[String],
        defaults: Defaults,
    ) -> Result<Self, String> {
        let volumes = db.volumes().map_err(|error| error.to_string())?;
        let directories = source_directories(&volumes);
        let mut exclusions = extra.to_vec();
        exclusions.extend([normalized(data), normalized(cache)]);
        Ok(Self {
            directories,
            exclusions,
            defaults,
            volumes,
        })
    }

    pub fn for_volume(&self, id: &str) -> Result<Self, String> {
        let volume = self
            .volumes
            .iter()
            .find(|volume| volume.id == id)
            .ok_or("Volume not found")?;
        let mut scope = self.clone();
        scope.directories = source_directories(std::slice::from_ref(volume));
        for other in &mut scope.volumes {
            if other.id != id {
                other.index_mode = FileIndexMode::Disabled;
            }
        }
        Ok(scope)
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.contains_entry(path, path.is_dir())
    }

    pub fn contains_entry(&self, path: &Path, is_dir: bool) -> bool {
        let path = normalized(path);
        self.directories.iter().any(|root| under(&path, root))
            && !self.excluded_entry(&path, is_dir)
    }

    pub fn excluded(&self, path: &str) -> bool {
        self.excluded_entry(path, Path::new(path).is_dir())
    }

    fn excluded_entry(&self, path: &str, is_dir: bool) -> bool {
        let path = normalized(Path::new(path));
        self.exclusions.iter().any(|root| under(&path, root))
            || self.defaults.excluded(&path, is_dir)
            || self
                .volumes
                .iter()
                .filter(|volume| under(&path, &volume.path))
                .max_by_key(|volume| (volume.path.len(), volume.is_available))
                .is_some_and(|volume| {
                    if !volume.is_available {
                        return true;
                    }
                    let listed = |dirs: &[String]| {
                        dirs.iter()
                            .any(|dir| under(&path, &absolute_directory(&volume.path, dir)))
                    };
                    match volume.index_mode {
                        FileIndexMode::Unconfigured | FileIndexMode::Disabled => true,
                        FileIndexMode::Allowlist => !listed(&volume.included_dirs),
                        FileIndexMode::Blocklist => listed(&volume.excluded_dirs),
                    }
                })
    }
}

pub fn source_directories(volumes: &[VolumeRow]) -> Vec<String> {
    let mut directories = Vec::new();
    for volume in volumes
        .iter()
        .filter(|volume| volume.is_available && !volume.path.contains("://"))
    {
        match volume.index_mode {
            FileIndexMode::Unconfigured | FileIndexMode::Disabled => {}
            FileIndexMode::Blocklist => directories.push(volume.path.clone()),
            FileIndexMode::Allowlist => directories.extend(
                volume
                    .included_dirs
                    .iter()
                    .map(|dir| absolute_directory(&volume.path, dir)),
            ),
        }
    }
    directories.sort();
    directories.dedup();
    directories
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/files/index_scope.rs"]
mod tests;
