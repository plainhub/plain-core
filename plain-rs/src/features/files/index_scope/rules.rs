use super::paths::{component_eq, normalized, under};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::path::Path;

#[derive(Clone)]
pub struct Defaults {
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

pub(super) fn resolve_defaults() -> Defaults {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let home = crate::infrastructure::platform::system::consts::home_directory();
    let mut excluded_directories: Vec<String> = Vec::new();
    #[cfg(target_os = "linux")]
    {
        excluded_directories.extend(
            [
                "/proc", "/sys", "/dev", "/run", "/tmp", "/usr", "/etc", "/var", "/boot", "/opt",
                "/lib", "/lib32", "/lib64", "/bin", "/sbin", "/snap",
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
                    .map(std::path::PathBuf::from)
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
    excluded_directories.sort();
    excluded_directories.dedup();
    Defaults {
        excluded_directories,
        excluded_names: [
            ".cache",
            ".config",
            ".local",
            ".git",
            "node_modules",
            "target",
            "build",
            "dist",
            "out",
            ".plain-trash",
            "lost+found",
            "$Recycle.Bin",
            "System Volume Information",
        ]
        .map(str::to_owned)
        .to_vec(),
    }
}

impl Defaults {
    pub(super) fn excluded(&self, path: &str, is_dir: bool) -> bool {
        #[cfg(target_os = "macos")]
        if macos_application_directory(path) {
            return true;
        }
        self.excluded_directories
            .iter()
            .any(|root| under(path, root))
            || (if is_dir {
                path
            } else {
                path.rsplit_once('/').map_or("", |(parent, _)| parent)
            })
            .split('/')
            .any(|part| {
                part.starts_with('.')
                    || self
                        .excluded_names
                        .iter()
                        .any(|name| component_eq(part, name))
            })
    }
}

#[cfg(target_os = "macos")]
const CLOUD_DOCUMENTS: &str = "Library/Mobile Documents/com~apple~CloudDocs";

#[cfg(target_os = "macos")]
pub(crate) fn macos_cloud_documents_ancestor(path: &str) -> bool {
    path.strip_prefix("/Users/")
        .and_then(|path| path.split_once('/'))
        .is_some_and(|(_, relative)| under(CLOUD_DOCUMENTS, relative))
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_application_directory(path: &str) -> bool {
    path.strip_prefix("/Users/")
        .and_then(|path| path.split_once('/'))
        .is_some_and(|(_, path)| {
            under(path, "Library") && !under(path, CLOUD_DOCUMENTS) && !under(CLOUD_DOCUMENTS, path)
        })
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_cloud_documents_root(path: &str) -> Option<&Path> {
    let profile = path.strip_prefix("/Users/")?.split_once('/')?.0;
    let length = "/Users/".len() + profile.len() + 1 + CLOUD_DOCUMENTS.len();
    let root = path.get(..length)?;
    (root.ends_with(CLOUD_DOCUMENTS) && under(path, root)).then(|| Path::new(root))
}
