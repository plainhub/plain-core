pub const MEDIA_DIRS: &[&str] = &[];
pub const MEDIA_DIR_SUFFIXES: &[&str] = &[];
pub const PROFILE_DIRS: &[super::ProfileDir] = &[];

pub fn index_directories() -> Vec<String> {
    Vec::new()
}

pub fn media_private_directory(_path: &str) -> bool {
    false
}
