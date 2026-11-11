use crate::anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    all(unix, any(test, feature = "test-seams"))
))]
pub mod linux;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    test,
    all(unix, feature = "test-seams")
))]
mod mounts;
#[cfg(all(unix, any(test, feature = "test-seams")))]
#[path = "../../../../tests/plain-libs/support/volume_mounts.rs"]
pub mod test_mounts;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub identity: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct Unavailable;

impl std::fmt::Display for Unavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Filesystem volume is unavailable")
    }
}

impl std::error::Error for Unavailable {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub volume: Volume,
    pub path: PathBuf,
}

pub fn locate(path: &Path) -> Result<Location> {
    ensure!(path.is_absolute(), "Absolute file path required");
    let mut existing = if std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        path.parent().context("Symlink parent is unavailable")?
    } else {
        path
    };
    loop {
        match std::fs::metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                existing = existing
                    .parent()
                    .context("Containing volume is unavailable")?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let canonical = existing.canonicalize()?;
    let mut location = native_location(canonical)?;
    let suffix = path.strip_prefix(existing)?;
    if !suffix.as_os_str().is_empty() {
        location.path.push(suffix);
    }
    ensure!(
        !location.volume.identity.is_empty() && location.volume.path.is_absolute(),
        "Invalid native volume facts"
    );
    location
        .path
        .strip_prefix(&location.volume.path)
        .context("File is outside its volume root")?;
    Ok(location)
}

fn native_location(canonical: PathBuf) -> Result<Location> {
    #[cfg(all(unix, any(test, feature = "test-seams")))]
    if let Some(volume) = test_mounts::containing(&canonical) {
        return Ok(Location {
            volume: volume?,
            path: canonical,
        });
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    return apple::containing(&canonical);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    return Ok(Location {
        volume: linux::containing(&canonical)?,
        path: canonical,
    });
    #[cfg(target_os = "windows")]
    return Ok(Location {
        volume: windows::containing(&canonical)?,
        path: PathBuf::from(windows_path(
            canonical.to_str().context("File path is not Unicode")?,
        )),
    });
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android",
        target_os = "windows"
    )))]
    Err(crate::anyhow::anyhow!(
        "Native filesystem volume identity is unavailable"
    ))
}

fn share_identity(source: &str) -> Result<String> {
    let source = source.trim_start_matches('/');
    let (authority, share) = source
        .split_once('/')
        .context("SMB share identity is unavailable")?;
    let server = authority
        .rsplit_once('@')
        .map_or(authority, |(_, server)| server);
    let share = share.trim_end_matches('/');
    ensure!(
        !server.is_empty() && !share.is_empty(),
        "Invalid SMB share identity"
    );
    Ok(format!("SHARE:{}/{}", server.to_lowercase(), share))
}

#[cfg(any(target_os = "windows", test))]
fn windows_path(path: &str) -> String {
    let path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
    };
    path.replace('\\', "/")
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/platform/volumes/mod.rs"]
mod tests;
