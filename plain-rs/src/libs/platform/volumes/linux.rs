use super::*;
use std::os::unix::fs::MetadataExt;

use super::mounts::{Mount, parse};

pub(crate) fn mounts() -> Result<Vec<Mount>> {
    parse(&std::fs::read_to_string("/proc/self/mountinfo")?)
}

fn filesystem_uuid(device: &str) -> Result<Option<String>> {
    let entries = match std::fs::read_dir("/dev/disk/by-uuid") {
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
        if format!(
            "{}:{}",
            libc::major(metadata.rdev()),
            libc::minor(metadata.rdev())
        ) == device
        {
            return Ok(Some(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Volume UUID is not UTF-8"))?,
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
    let identity = if matches!(mount.fs_type.as_str(), "cifs" | "smb3") {
        share_identity(&mount.source)?
    } else if let Some(uuid) = filesystem_uuid(&mount.device)? {
        format!("UUID:{}", uuid.to_uppercase())
    } else {
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
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
