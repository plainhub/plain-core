#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod other;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use other::*;
#[cfg(target_os = "windows")]
pub use windows::*;

use super::paths::under;

#[derive(Clone, Copy, Debug)]
pub struct ProfileDir {
    pub profiles_dir: &'static str,
    pub excluded_dir: &'static str,
    pub except_dirs: &'static [&'static str],
}

impl ProfileDir {
    fn relative_path<'a>(&self, path: &'a str) -> Option<&'a str> {
        path.strip_prefix(self.profiles_dir)?
            .split_once('/')
            .map(|(_, relative)| relative)
    }

    pub fn excludes(&self, path: &str) -> bool {
        self.relative_path(path).is_some_and(|relative| {
            under(relative, self.excluded_dir)
                && !self
                    .except_dirs
                    .iter()
                    .any(|exception| under(relative, exception) || under(exception, relative))
        })
    }
}

pub fn excluded_profile_directory(path: &str) -> bool {
    PROFILE_DIRS.iter().any(|rule| rule.excludes(path))
}

#[cfg(not(target_os = "windows"))]
pub fn media_system_directory(path: &str) -> bool {
    MEDIA_DIRS.iter().any(|root| under_root(path, root))
}

pub fn media_package_component(component: &str) -> bool {
    let bytes = component.as_bytes();
    MEDIA_DIR_SUFFIXES.iter().any(|suffix| {
        bytes
            .len()
            .checked_sub(suffix.len())
            .is_some_and(|start| bytes[start..].eq_ignore_ascii_case(suffix.as_bytes()))
    })
}

pub fn under_root(path: &str, root: &str) -> bool {
    path == root || path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'/')
}

pub fn under_root_case_insensitive(path: &str, root: &str) -> bool {
    path.eq_ignore_ascii_case(root)
        || path
            .get(..root.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(root))
            && path.as_bytes().get(root.len()) == Some(&b'/')
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/files/index_scope/platform.rs"]
mod tests;
