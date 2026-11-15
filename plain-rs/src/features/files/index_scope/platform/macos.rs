use super::super::paths::under;
use super::ProfileDir;
use std::path::Path;

pub const INDEX_DIRS: &[&str] = &[
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
];
pub const MEDIA_DIRS: &[&str] = &[
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
pub const MEDIA_DIR_SUFFIXES: &[&str] = &[".photoslibrary", ".app", ".bundle", ".framework"];
pub const CLOUD_DOCUMENTS: &str = "Library/Mobile Documents/com~apple~CloudDocs";
pub const PROFILE_DIRS: &[ProfileDir] = &[ProfileDir {
    profiles_dir: "/Users/",
    excluded_dir: "Library",
    except_dirs: &[CLOUD_DOCUMENTS],
}];

pub fn index_directories() -> Vec<String> {
    INDEX_DIRS.iter().map(|dir| (*dir).to_owned()).collect()
}

pub use super::excluded_profile_directory as media_private_directory;

pub(crate) fn macos_cloud_documents_ancestor(path: &str) -> bool {
    PROFILE_DIRS[0]
        .relative_path(path)
        .is_some_and(|relative| under(CLOUD_DOCUMENTS, relative))
}

pub(crate) fn macos_cloud_documents_root(path: &str) -> Option<&Path> {
    let profiles_dir = PROFILE_DIRS[0].profiles_dir;
    let profile = path.strip_prefix(profiles_dir)?.split_once('/')?.0;
    let length = profiles_dir.len() + profile.len() + 1 + CLOUD_DOCUMENTS.len();
    let root = path.get(..length)?;
    (root.ends_with(CLOUD_DOCUMENTS) && under(path, root)).then(|| Path::new(root))
}
