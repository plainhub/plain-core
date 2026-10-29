use std::path::Path;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use std::sync::OnceLock;

#[cfg(target_os = "macos")]
const EXCLUDED_SYSTEM_ROOTS: &[&str] = &[
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
    "/proc",
];

#[cfg(target_os = "linux")]
const EXCLUDED_SYSTEM_ROOTS: &[&str] = &[
    "/proc", "/sys", "/dev", "/run", "/tmp", "/snap", "/usr", "/etc", "/var", "/boot", "/opt",
    "/srv", "/lib", "/lib32", "/lib64", "/bin", "/sbin",
];

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
const EXCLUDED_SYSTEM_ROOTS: &[&str] = &[];

const EXCLUDED_DIR_NAMES: &[&str] = &["node_modules"];

const PROFILE_CACHE_NAMES: &[&str] = &["cache", "caches", "gpucache", "code cache", "cachestorage"];

pub fn is_file_excluded_at(
    path: &str,
    data_dir: &Path,
    cache_dir: &Path,
    extra_roots: &[String],
) -> bool {
    let path = Path::new(path);
    path.starts_with(data_dir)
        || path.starts_with(cache_dir)
        || path
            .components()
            .any(|part| part.as_os_str() == ".plain-trash")
        || extra_roots.iter().any(|root| path.starts_with(root))
}

pub fn is_media_excluded_at(
    path: &str,
    data_dir: &Path,
    cache_dir: &Path,
    extra_roots: &[String],
) -> bool {
    let p = path.replace('\\', "/");
    if is_media_excluded_name_or_root(&p, data_dir, cache_dir, extra_roots) {
        return true;
    }
    let mut ancestor = Some(Path::new(&p));
    while let Some(dir) = ancestor {
        if dir.join(".nomedia").is_file()
            || (is_native_hidden(dir) && !is_cloud_drive_ancestor(&p, dir))
        {
            return true;
        }
        ancestor = dir.parent();
    }
    false
}

#[cfg(target_os = "macos")]
fn is_cloud_drive_ancestor(path: &str, ancestor: &Path) -> bool {
    let Some(cloud) = cloud_drive_path() else {
        return false;
    };
    Path::new(path).starts_with(cloud) && cloud.starts_with(ancestor)
}

#[cfg(target_os = "macos")]
fn cloud_drive_path() -> Option<&'static std::path::PathBuf> {
    static CLOUD: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    CLOUD
        .get_or_init(|| {
            std::env::var_os("HOME")
                .map(|home| Path::new(&home).join("Library/Mobile Documents/com~apple~CloudDocs"))
        })
        .as_ref()
}

#[cfg(not(target_os = "macos"))]
fn is_cloud_drive_ancestor(_path: &str, _ancestor: &Path) -> bool {
    false
}

pub fn is_media_excluded_name_or_root(
    p: &str,
    data_dir: &Path,
    cache_dir: &Path,
    extra_roots: &[String],
) -> bool {
    let mut follows_user_data = false;
    if Path::new(&p).components().any(|c| {
        c.as_os_str().to_str().is_some_and(|comp| {
            let profile_cache = follows_user_data
                && PROFILE_CACHE_NAMES
                    .iter()
                    .any(|name| comp.eq_ignore_ascii_case(name));
            follows_user_data = comp.eq_ignore_ascii_case("User Data");
            comp.starts_with('.')
                || EXCLUDED_DIR_NAMES
                    .iter()
                    .any(|name| comp.eq_ignore_ascii_case(name))
                || is_excluded_package_component(comp)
                || profile_cache
        })
    }) {
        return true;
    }
    if is_system_root(p) {
        return true;
    }
    if is_platform_private_root(p) {
        return true;
    }
    for root in extra_roots {
        if under_root(&p, root.trim_end_matches('/')) {
            return true;
        }
    }
    for dir in [data_dir, cache_dir] {
        let s = dir.to_string_lossy().replace('\\', "/");
        if under_root(&p, s.trim_end_matches('/')) {
            return true;
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn is_platform_private_root(path: &str) -> bool {
    static LIBRARY: OnceLock<Option<String>> = OnceLock::new();
    let library = LIBRARY.get_or_init(|| {
        std::env::var_os("HOME").map(|home| {
            Path::new(&home)
                .join("Library")
                .to_string_lossy()
                .to_string()
        })
    });
    library.as_ref().is_some_and(|library| {
        under_root(path, library)
            && !cloud_drive_path().is_some_and(|cloud| Path::new(path).starts_with(cloud))
    })
}

#[cfg(target_os = "windows")]
fn is_platform_private_root(path: &str) -> bool {
    static APP_DATA: OnceLock<Option<String>> = OnceLock::new();
    let app_data = APP_DATA.get_or_init(|| {
        std::env::var_os("USERPROFILE").map(|home| {
            Path::new(&home)
                .join("AppData")
                .to_string_lossy()
                .replace('\\', "/")
        })
    });
    app_data
        .as_ref()
        .is_some_and(|app_data| under_root_case_insensitive(path, app_data))
}

#[cfg(target_os = "linux")]
fn is_platform_private_root(path: &str) -> bool {
    static SNAP: OnceLock<Option<String>> = OnceLock::new();
    let snap = SNAP.get_or_init(|| {
        std::env::var_os("HOME")
            .map(|home| Path::new(&home).join("snap").to_string_lossy().to_string())
    });
    snap.as_ref().is_some_and(|snap| under_root(path, snap))
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn is_platform_private_root(_path: &str) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn is_native_hidden(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_attributes() & (0x2 | 0x4) != 0)
}

#[cfg(target_os = "macos")]
pub fn is_native_hidden(path: &Path) -> bool {
    use std::os::macos::fs::MetadataExt;
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.st_flags() & 0x8000 != 0)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn is_native_hidden(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn is_excluded_package_component(component: &str) -> bool {
    [".photoslibrary", ".app", ".bundle", ".framework"]
        .iter()
        .any(|suffix| component.to_ascii_lowercase().ends_with(suffix))
}

#[cfg(not(target_os = "macos"))]
fn is_excluded_package_component(_component: &str) -> bool {
    false
}

#[cfg(target_os = "windows")]
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

#[cfg(target_os = "windows")]
fn is_system_root(path: &str) -> bool {
    static ROOTS: OnceLock<Vec<String>> = OnceLock::new();
    ROOTS
        .get_or_init(windows_system_roots)
        .iter()
        .any(|root| under_root_case_insensitive(path, root))
}

#[cfg(not(target_os = "windows"))]
fn is_system_root(path: &str) -> bool {
    EXCLUDED_SYSTEM_ROOTS
        .iter()
        .any(|root| under_root(path, root))
}

fn under_root(p: &str, root: &str) -> bool {
    p == root || p.starts_with(&format!("{root}/"))
}

pub fn under_root_case_insensitive(p: &str, root: &str) -> bool {
    p.eq_ignore_ascii_case(root)
        || p.get(..root.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(root))
            && p.as_bytes().get(root.len()) == Some(&b'/')
}

pub fn infer_type(name: &str) -> &'static str {
    let mime = plain_rs::utils::mime::mime_from_ext(name);
    if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("audio/") {
        "audio"
    } else if mime.starts_with("text/") || DOC_EXTRA_MIMES.contains(&mime) {
        "doc"
    } else {
        "other"
    }
}

const DOC_EXTRA_MIMES: [&str; 5] = [
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/javascript",
];

pub fn selected_roots(root: String, source_dirs: &[String]) -> Vec<std::path::PathBuf> {
    if root == "/" {
        let sources = source_dirs
            .iter()
            .filter(|dir| !dir.is_empty())
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>();
        if !sources.is_empty() {
            return sources;
        }
    }
    vec![std::path::PathBuf::from(root)]
}

#[cfg(test)]
#[path = "../../tests/unit/media/catalog_paths.rs"]
mod tests;
