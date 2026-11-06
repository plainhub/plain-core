use crate::features::files::index_scope;
use crate::infrastructure::db::{IndexMode, VolumeRow};
use std::path::Path;

pub fn set_directories(
    db: &crate::infrastructure::db::Db,
    directories: &[String],
) -> Result<(), String> {
    let directories = index_scope::validate_directories(directories.to_vec())?;
    let mut grouped = std::collections::BTreeMap::<String, Vec<String>>::new();
    for directory in directories {
        let location = plain_libs::platform::volumes::locate(Path::new(&directory))
            .map_err(|error| error.to_string())?;
        let id = db
            .register_volume(&location.path)
            .map_err(|error| error.to_string())?;
        let relative = crate::infrastructure::db::files::relative_path(
            location
                .path
                .strip_prefix(&location.volume.path)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        grouped.entry(id).or_default().push(relative);
    }
    let volumes = db.volumes().map_err(|error| error.to_string())?;
    for (id, directories) in grouped {
        let directories = index_scope::relative_directories(directories)?;
        let volume = volumes
            .iter()
            .find(|volume| volume.id == id)
            .ok_or("Volume not found")?;
        if !db
            .set_volume_index(
                &id,
                IndexMode::Allowlist,
                &directories,
                &volume.excluded_dirs,
            )
            .map_err(|error| error.to_string())?
        {
            return Err("Volume not found".into());
        }
    }
    Ok(())
}

pub(super) fn volume_directories(
    volume: &VolumeRow,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut directories = Vec::new();
    for path in index_scope::validate_directories(paths)? {
        let relative = if volume.is_available {
            let location = plain_libs::platform::volumes::locate(Path::new(&path))
                .map_err(|error| error.to_string())?;
            if location.volume.identity != volume.identity {
                return Err("Directory belongs to another volume".into());
            }
            crate::infrastructure::db::files::relative_path(
                location
                    .path
                    .strip_prefix(&location.volume.path)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?
        } else {
            if !index_scope::under(&path, &volume.path) {
                return Err("Directory belongs to another volume".into());
            }
            path[volume.path.trim_end_matches('/').len()..]
                .trim_start_matches('/')
                .to_owned()
        };
        directories.push(relative);
    }
    index_scope::relative_directories(directories)
}
