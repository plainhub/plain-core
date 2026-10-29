use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) mod linux;
#[cfg(any(target_os = "linux", target_os = "android", test))]
mod mounts;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub identity: String,
    pub path: PathBuf,
}

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
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let mut location = apple::containing(&canonical)?;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let mut location = Location {
        volume: linux::containing(&canonical)?,
        path: canonical,
    };
    #[cfg(target_os = "windows")]
    let mut location = Location {
        volume: windows::containing(&canonical)?,
        path: PathBuf::from(windows_path(
            canonical.to_str().context("File path is not Unicode")?,
        )),
    };
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android",
        target_os = "windows"
    )))]
    return Err(anyhow::anyhow!(
        "Native filesystem volume identity is unavailable"
    ));
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "linux",
        target_os = "android",
        target_os = "windows"
    ))]
    {
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
#[path = "../../../tests/unit/platform/volumes/mod.rs"]
mod tests;
