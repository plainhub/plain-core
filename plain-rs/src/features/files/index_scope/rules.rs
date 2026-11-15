use super::paths::{component_eq, normalized, under};

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
    let mut excluded_directories = super::platform::index_directories();
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
        if super::platform::excluded_profile_directory(path) {
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
