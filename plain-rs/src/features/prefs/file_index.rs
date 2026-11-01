use super::{Prefs, pref_keys};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Defaults {
    pub scan_directories: Vec<String>,
    pub excluded_directories: Vec<String>,
    pub excluded_names: Vec<String>,
}

pub fn defaults() -> Defaults {
    static DEFAULTS: std::sync::OnceLock<Defaults> = std::sync::OnceLock::new();
    DEFAULTS.get_or_init(resolve_defaults).clone()
}

impl Default for Defaults {
    fn default() -> Self {
        defaults()
    }
}

fn resolve_defaults() -> Defaults {
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let mut scan_directories: Vec<_> = if cfg!(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos"
    )) {
        home.iter().map(|path| normalized(path)).collect()
    } else {
        Vec::new()
    };
    let mut excluded_directories: Vec<String> = Vec::new();
    #[cfg(target_os = "linux")]
    {
        excluded_directories.extend(
            [
                "/proc", "/sys", "/dev", "/run", "/tmp", "/usr", "/etc", "/var", "/boot", "/opt",
                "/srv", "/lib", "/lib32", "/lib64", "/bin", "/sbin", "/snap",
            ]
            .map(str::to_owned),
        );
        if let Some(home) = &home {
            for (key, fallback) in [
                ("XDG_DATA_HOME", ".local/share"),
                ("XDG_CONFIG_HOME", ".config"),
                ("XDG_CACHE_HOME", ".cache"),
                ("XDG_STATE_HOME", ".local/state"),
            ] {
                let path = std::env::var_os(key)
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .unwrap_or_else(|| home.join(fallback));
                excluded_directories.push(normalized(&path));
            }
            excluded_directories.push(normalized(&home.join("snap")));
        }
    }
    #[cfg(target_os = "macos")]
    {
        excluded_directories.extend(
            [
                "/System",
                "/Library",
                "/Applications",
                "/private",
                "/usr",
                "/bin",
                "/sbin",
                "/var",
                "/tmp",
                "/dev",
                "/cores",
            ]
            .map(str::to_owned),
        );
        if let Some(home) = &home {
            let cloud = home.join("Library/Mobile Documents/com~apple~CloudDocs");
            if cloud.is_dir() {
                scan_directories.push(normalized(&cloud));
            }
            excluded_directories.push(normalized(&home.join("Library")));
        }
    }
    #[cfg(target_os = "windows")]
    {
        for key in [
            "SystemRoot",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
            "APPDATA",
            "LOCALAPPDATA",
            "TEMP",
            "TMP",
        ] {
            if let Some(path) = std::env::var_os(key) {
                excluded_directories.push(normalized(Path::new(&path)));
            }
        }
        if let Some(home) = &home {
            excluded_directories.push(normalized(&home.join("AppData")));
        }
    }
    excluded_directories.push(normalized(&std::env::temp_dir()));
    scan_directories.sort();
    scan_directories.dedup();
    excluded_directories.sort();
    excluded_directories.dedup();
    Defaults {
        scan_directories,
        excluded_directories,
        excluded_names: [
            ".git",
            "node_modules",
            "target",
            "build",
            "dist",
            "out",
            ".plain-trash",
            "$Recycle.Bin",
            "System Volume Information",
        ]
        .map(str::to_owned)
        .to_vec(),
    }
}

impl Defaults {
    fn excluded(&self, path: &str, is_dir: bool) -> bool {
        let cloud = self
            .scan_directories
            .iter()
            .any(|root| root.contains("/Library/Mobile Documents/") && under(path, root));
        self.excluded_directories
            .iter()
            .any(|root| !(cloud && root.ends_with("/Library")) && under(path, root))
            || (if is_dir {
                path
            } else {
                path.rsplit_once('/').map_or("", |(parent, _)| parent)
            })
            .split('/')
            .any(|part| {
                self.excluded_names
                    .iter()
                    .any(|name| component_eq(part, name))
            })
    }
}

#[derive(Clone)]
pub struct Scope {
    pub directories: Vec<String>,
    pub exclusions: Vec<String>,
    pub defaults: Defaults,
}

impl Scope {
    pub fn load(
        prefs: &Prefs,
        data: &Path,
        cache: &Path,
        extra: &[String],
        defaults: Defaults,
    ) -> Result<Self, String> {
        let directories = source_directories(prefs, &defaults)?;
        let mut exclusions = prefs
            .get::<Vec<String>>(pref_keys::FILE_INDEX_EXCLUDED_DIRS)
            .map_err(|e| e.to_string())?
            .unwrap_or_default();
        exclusions.extend(extra.iter().cloned());
        exclusions.extend([normalized(data), normalized(cache)]);
        Ok(Self {
            directories: validate_directories(directories)?,
            exclusions: validate_directories(exclusions)?,
            defaults,
        })
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
    }
}

pub fn source_directories(prefs: &Prefs, defaults: &Defaults) -> Result<Vec<String>, String> {
    let mut directories = defaults.scan_directories.clone();
    directories.extend(
        prefs
            .get::<Vec<String>>(pref_keys::MEDIA_SOURCE_DIRS)
            .map_err(|error| error.to_string())?
            .unwrap_or_default(),
    );
    validate_directories(directories)
}

pub fn validate_directories(paths: Vec<String>) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for path in paths {
        let path = Path::new(&path);
        if !path.is_absolute() {
            return Err("Absolute index directory required".into());
        }
        if path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
        {
            return Err("Index directories must not contain parent components".into());
        }
        match std::fs::metadata(path) {
            Ok(metadata) if !metadata.is_dir() => return Err("Index directory required".into()),
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) => {}
            Err(error) => return Err(error.to_string()),
        }
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let path = match path.canonicalize() {
            Ok(path) => path,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                ) =>
            {
                path
            }
            Err(error) => return Err(error.to_string()),
        };
        result.push(normalized(&path));
    }
    result.sort();
    result.dedup();
    Ok(result)
}

pub fn normalized(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = if let Some(unc) = path.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else {
        path.strip_prefix("//?/").unwrap_or(&path).to_owned()
    };
    if path == "/" || (path.len() == 3 && path.as_bytes()[1] == b':' && path.ends_with('/')) {
        path
    } else {
        path.trim_end_matches('/').to_owned()
    }
}

pub fn under(path: &str, root: &str) -> bool {
    if root == "/" {
        return path.starts_with('/');
    }
    component_eq(path, root)
        || path
            .get(..root.len())
            .is_some_and(|head| component_eq(head, root))
            && (root.ends_with('/') || path.as_bytes().get(root.len()) == Some(&b'/'))
}

fn component_eq(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/prefs/file_index.rs"]
mod tests;
