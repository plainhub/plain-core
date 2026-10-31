use std::path::{Path, PathBuf};

pub fn unique_sibling(target: &Path) -> std::io::Result<PathBuf> {
    unique_sibling_excluding(target, |_| false)
}

pub fn unique_sibling_excluding(
    target: &Path,
    occupied: impl Fn(&Path) -> bool,
) -> std::io::Result<PathBuf> {
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    let base = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "UTF-8 filename required")
        })?;
    let (stem, extension) = match base.rfind('.') {
        Some(position) if position > 0 => (&base[..position], &base[position..]),
        _ => (base, ""),
    };
    let stem = stem
        .rsplit_once(" (")
        .and_then(|(name, suffix)| {
            suffix
                .strip_suffix(')')
                .filter(|index| {
                    !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                })
                .map(|_| name)
        })
        .unwrap_or(stem);
    let mut index = 0_u64;
    loop {
        let candidate = if index == 0 {
            target.to_path_buf()
        } else {
            parent.join(format!("{stem} ({index}){extension}"))
        };
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !occupied(&candidate) {
                    return Ok(candidate);
                }
            }
            Err(error) => return Err(error),
        }
        index = index
            .checked_add(1)
            .ok_or_else(|| std::io::Error::other("unique filename exhausted"))?;
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/utils/unique_path.rs"]
mod tests;
