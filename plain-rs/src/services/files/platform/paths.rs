use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

fn canonical(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("absolute file path required");
    }
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            value => {
                resolved.push(value.as_os_str());
                match resolved.canonicalize() {
                    Ok(path) => resolved = path,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        match std::fs::symlink_metadata(&resolved) {
                            Ok(_) => return Err(error.into()),
                            Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {}
                            Err(other) => return Err(other.into()),
                        }
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    Ok(resolved)
}

#[cfg(any(target_os = "android", test))]
fn validate_android(path: &Path) -> Result<()> {
    const PROTECTED: &[&str] = &[
        "/system",
        "/proc",
        "/sys",
        "/dev",
        "/data/data",
        "/data/app",
        "/data/system",
        "/data/misc",
        "/vendor",
        "/product",
        "/apex",
        "/oem",
        "/odm",
    ];
    if path
        .components()
        .filter(|part| matches!(part, Component::Normal(_)))
        .count()
        < 2
        || PROTECTED.iter().any(|root| path.starts_with(root))
    {
        bail!("Protected file path");
    }
    Ok(())
}

pub(super) fn external(
    paths: &[String],
    directory: &Path,
    cache: &Path,
    write: bool,
) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for path in paths {
        let resolved = canonical(Path::new(path))?;
        if resolved.starts_with(directory) || resolved.starts_with(cache) {
            continue;
        }
        if write {
            plain_rs::files::filesystem::deletion::validate(&resolved)?;
            #[cfg(target_os = "android")]
            {
                validate_android(Path::new(path))?;
                validate_android(&resolved)?;
            }
        }
        result.push(path.clone());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/files/platform/paths.rs"]
mod tests;
