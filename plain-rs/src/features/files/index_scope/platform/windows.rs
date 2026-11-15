use std::path::Path;
use std::sync::OnceLock;

pub const MEDIA_DIR_SUFFIXES: &[&str] = &[];
pub const PROFILE_DIRS: &[super::ProfileDir] = &[];

pub fn index_directories() -> Vec<String> {
    let mut directories = Vec::new();
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
            directories.push(super::super::normalized(Path::new(&path)));
        }
    }
    if let Some(home) = crate::infrastructure::platform::system::consts::home_directory() {
        directories.push(super::super::normalized(&home.join("AppData")));
    }
    directories
}

pub fn media_private_directory(path: &str) -> bool {
    static APP_DATA: OnceLock<Option<String>> = OnceLock::new();
    APP_DATA
        .get_or_init(|| {
            std::env::var_os("USERPROFILE").map(|home| {
                Path::new(&home)
                    .join("AppData")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
        })
        .as_ref()
        .is_some_and(|root| super::under_root_case_insensitive(path, root))
}

fn windows_system_roots() -> Vec<String> {
    let mut roots = Vec::new();
    for key in [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
    ] {
        if let Some(path) = std::env::var_os(key) {
            roots.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        if let Some(drive) = Path::new(&system_root).components().next() {
            let drive = drive.as_os_str().to_string_lossy();
            roots.extend(
                [
                    "$Recycle.Bin",
                    "System Volume Information",
                    "Recovery",
                    "PerfLogs",
                ]
                .into_iter()
                .map(|name| format!("{drive}/{name}")),
            );
        }
    }
    roots
}

pub fn media_system_directory(path: &str) -> bool {
    static ROOTS: OnceLock<Vec<String>> = OnceLock::new();
    ROOTS
        .get_or_init(windows_system_roots)
        .iter()
        .any(|root| super::under_root_case_insensitive(path, root))
}
