pub const MEDIA_DIRS: &[&str] = &[
    "/proc", "/sys", "/dev", "/run", "/tmp", "/snap", "/usr", "/etc", "/var", "/boot", "/opt",
    "/lib", "/lib32", "/lib64", "/bin", "/sbin",
];
pub const INDEX_DIRS: &[&str] = &[
    "/proc", "/sys", "/dev", "/run", "/tmp", "/usr", "/etc", "/var", "/boot", "/opt", "/lib",
    "/lib32", "/lib64", "/bin", "/sbin", "/snap",
];
pub const MEDIA_DIR_SUFFIXES: &[&str] = &[];
pub const PROFILE_DIRS: &[super::ProfileDir] = &[];

pub fn index_directories() -> Vec<String> {
    let mut directories: Vec<String> = INDEX_DIRS.iter().map(|dir| (*dir).to_owned()).collect();
    if let Some(home) = crate::infrastructure::platform::system::consts::home_directory() {
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
            directories.push(super::super::normalized(&path));
        }
        directories.push(super::super::normalized(&home.join("snap")));
    }
    directories
}

pub fn media_private_directory(path: &str) -> bool {
    static SNAP: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    SNAP.get_or_init(|| {
        std::env::var_os("HOME").map(|home| {
            std::path::Path::new(&home)
                .join("snap")
                .to_string_lossy()
                .to_string()
        })
    })
    .as_ref()
    .is_some_and(|root| super::under_root(path, root))
}
