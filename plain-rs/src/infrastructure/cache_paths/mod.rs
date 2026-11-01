use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub mod system;

#[derive(Clone, Debug)]
pub struct CachePaths {
    root: Arc<PathBuf>,
}
impl CachePaths {
    pub fn new(root: &Path) -> Self {
        Self {
            root: Arc::new(root.to_owned()),
        }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn image_cache(&self) -> PathBuf {
        self.root.join("image_cache")
    }
    pub fn video_cache(&self) -> PathBuf {
        self.root.join("video_cache")
    }
    pub fn shared_previews(&self) -> PathBuf {
        self.root.join("share_open")
    }
    pub fn app_updates(&self) -> PathBuf {
        self.root.join("updates")
    }
    pub fn pdf_previews(&self) -> PathBuf {
        self.root.join("preview_pdf")
    }
    pub fn crash_report_export(&self) -> PathBuf {
        self.root.join("crash_report.txt")
    }
    pub fn initialize(&self, data_directory: &Path) -> std::io::Result<()> {
        let invalid = |message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message);
        if !self.root.is_absolute() || !data_directory.is_absolute() {
            return Err(invalid("Absolute application directories required"));
        }
        if std::fs::symlink_metadata(self.root()).is_ok_and(|metadata| metadata.is_symlink()) {
            return Err(invalid("Cache directory cannot be a symbolic link"));
        }
        std::fs::create_dir_all(self.root())?;
        let root = self.root.canonicalize()?;
        let data = data_directory.canonicalize()?;
        if data.starts_with(&root) {
            return Err(invalid(
                "Cache directory cannot contain persistent application data",
            ));
        }
        for path in [
            self.image_cache(),
            self.video_cache(),
            self.shared_previews(),
            self.app_updates(),
        ] {
            if std::fs::symlink_metadata(path)
                .is_ok_and(|metadata| metadata.is_symlink() || !metadata.is_dir())
            {
                return Err(invalid("Invalid application cache directory"));
            }
        }
        std::fs::create_dir_all(self.shared_previews())?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/cache_paths/mod.rs"]
mod tests;
