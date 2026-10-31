#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
use sha2::{Digest, Sha256};
#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn resolve(data_directory: &Path) -> Result<PathBuf, String> {
    if !data_directory.is_absolute() {
        return Err("Absolute data directory required".into());
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Err("SDK cache directory required at startup".into());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let home = std::env::var_os("HOME");
        let cache_home = std::env::var_os("XDG_CACHE_HOME");
        let local_data = std::env::var_os("LOCALAPPDATA");
        let family = if cfg!(target_os = "macos") {
            Family::Apple
        } else if cfg!(windows) {
            Family::Windows
        } else {
            Family::Xdg
        };
        let base = base(
            family,
            home.as_deref(),
            cache_home.as_deref(),
            local_data.as_deref(),
        )?;
        Ok(scoped(&base, data_directory))
    }
}

#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
#[derive(Clone, Copy)]
enum Family {
    Xdg,
    Apple,
    Windows,
}

#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
fn absolute(value: Option<&OsStr>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}
#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
fn base(
    family: Family,
    home: Option<&OsStr>,
    cache_home: Option<&OsStr>,
    local_data: Option<&OsStr>,
) -> Result<PathBuf, String> {
    let root = match family {
        Family::Xdg => {
            absolute(cache_home).or_else(|| absolute(home).map(|root| root.join(".cache")))
        }
        Family::Apple => absolute(home).map(|root| root.join("Library/Caches")),
        Family::Windows => absolute(local_data),
    };
    root.ok_or("System cache directory unavailable".into())
}
#[cfg(any(test, not(any(target_os = "android", target_os = "ios"))))]
fn scoped(base: &Path, directory: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in directory.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    let key = Sha256::digest(normalized.as_os_str().as_encoded_bytes());
    base.join("plain")
        .join(crate::utils::hex::bytes_to_hex(&key[..16]))
}

#[cfg(test)]
#[path = "../../tests/unit/cache_paths/system.rs"]
mod tests;
