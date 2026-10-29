use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MountInfoEntry {
    pub mount_point: String,
}

#[cfg(target_os = "linux")]
pub fn read_mountinfo() -> anyhow::Result<Vec<MountInfoEntry>> {
    let mounts = super::volumes::linux::mounts()?;
    anyhow::ensure!(!mounts.is_empty(), "No mount information entries");
    Ok(mounts
        .into_iter()
        .map(|mount| MountInfoEntry {
            mount_point: mount.path.to_string_lossy().into_owned(),
        })
        .collect())
}

#[cfg(not(target_os = "linux"))]
pub fn read_mountinfo() -> anyhow::Result<Vec<MountInfoEntry>> {
    let entries: Vec<_> = sysinfo::Disks::new_with_refreshed_list()
        .iter()
        .map(|disk| MountInfoEntry {
            mount_point: disk.mount_point().to_string_lossy().into_owned(),
        })
        .collect();
    if entries.is_empty() {
        anyhow::bail!("no mounted volumes");
    }
    Ok(entries)
}

fn find_best_mount_point(entries: &[MountInfoEntry], abs_path: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for e in entries {
        let mp = e.mount_point.trim();
        if mp.is_empty() {
            continue;
        }
        let matched = Path::new(abs_path).starts_with(Path::new(mp));
        if matched {
            match &best {
                None => best = Some(mp.to_string()),
                Some(b) if mp.len() > b.len() => best = Some(mp.to_string()),
                _ => {}
            }
        }
    }
    best
}

pub fn resolve_mount_point<P: AsRef<Path>>(abs_path: P) -> anyhow::Result<String> {
    let abs = abs_path.as_ref().clean_path();
    if abs.as_os_str().is_empty() || abs == PathBuf::from(".") {
        anyhow::bail!("invalid path");
    }
    let abs_str = match abs.to_str() {
        Some(s) => s.to_string(),
        None => anyhow::bail!("invalid utf-8 path"),
    };

    let entries = read_mountinfo()?;
    if let Some(best) = find_best_mount_point(&entries, &abs_str) {
        return Ok(best);
    }

    if let Ok(real) = std::fs::canonicalize(&abs) {
        let real_str = real.to_string_lossy().to_string();
        if real_str != abs_str {
            if let Some(best) = find_best_mount_point(&entries, &real_str) {
                return Ok(best);
            }
        }
    }
    anyhow::bail!("unable to resolve mountpoint")
}

pub trait CleanPath {
    fn clean_path(&self) -> PathBuf;
}
impl CleanPath for Path {
    fn clean_path(&self) -> PathBuf {
        path_clean(self)
    }
}
impl CleanPath for PathBuf {
    fn clean_path(&self) -> PathBuf {
        path_clean(self.as_path())
    }
}

fn path_clean(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else if !p.is_absolute() {
                    out.push(comp.as_os_str());
                }
            }
            _ => out.push(comp.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/unit/platform/mountinfo.rs"]
mod tests;
