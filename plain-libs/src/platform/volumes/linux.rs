use super::*;
use std::os::unix::fs::MetadataExt;

use super::mounts::{Mount, parse};

pub fn mounts() -> Result<Vec<Mount>> {
    parse(&std::fs::read_to_string("/proc/self/mountinfo")?)
}

fn device_number(metadata: &std::fs::Metadata) -> String {
    let device = metadata.rdev() as libc::dev_t;
    format!("{}:{}", libc::major(device), libc::minor(device))
}

fn filesystem_uuid(mount: &Mount, directory: &Path) -> Result<Option<String>> {
    // Anonymous mount devices, such as Btrfs subvolumes, use the source block device number.
    let device = if mount.device.starts_with("0:") && mount.source.starts_with("/dev/") {
        match std::fs::metadata(&mount.source) {
            Ok(metadata) => device_number(&metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
    } else {
        mount.device.clone()
    };
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let metadata = match std::fs::metadata(entry.path()) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if device_number(&metadata) == device {
            return Ok(Some(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| crate::anyhow::anyhow!("Volume UUID is not UTF-8"))?,
            ));
        }
    }
    Ok(None)
}

pub(super) fn containing(path: &Path) -> Result<Volume> {
    let mount = mounts()?
        .into_iter()
        .filter(|mount| path.starts_with(&mount.path))
        .max_by_key(|mount| mount.path.components().count())
        .context("No containing filesystem mount")?;
    volume(
        mount,
        Path::new("/dev/disk/by-uuid"),
        Path::new("/proc/sys/kernel/random/boot_id"),
    )
}

fn volume(mount: Mount, uuid_directory: &Path, boot_id: &Path) -> Result<Volume> {
    let identity = if matches!(mount.fs_type.as_str(), "cifs" | "smb3") {
        share_identity(&mount.source)?
    } else if let Some(uuid) = filesystem_uuid(&mount, uuid_directory)? {
        format!("UUID:{}", uuid.to_uppercase())
    } else if mount.source.starts_with("/dev/") {
        // Detached block mounts can outlive their device nodes and UUID links.
        return Err(Unavailable.into());
    } else {
        let boot = std::fs::read_to_string(boot_id)?;
        format!("SESSION:{}:{}", boot.trim(), mount.id)
    };
    let identity = if mount.root == "/" {
        identity
    } else {
        format!("{identity}:ROOT:{}", mount.root)
    };
    Ok(Volume {
        identity,
        path: mount.path,
    })
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/platform/volumes/linux.rs"]
mod tests;
