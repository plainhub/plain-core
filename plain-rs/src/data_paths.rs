use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DataPaths {
    root: PathBuf,
}
impl DataPaths {
    pub fn new(directory: &Path) -> Self {
        Self {
            root: directory.to_path_buf(),
        }
    }
    pub fn lock(&self) -> PathBuf {
        self.root.join(".core.lock")
    }
    pub fn pending_restore(&self) -> PathBuf {
        self.root.join(".restore-pending.json")
    }
    pub fn database(&self) -> PathBuf {
        self.root.join("plain.db")
    }
    pub fn prefs(&self) -> PathBuf {
        self.root.join("prefs.json")
    }
    pub fn note_images(&self) -> PathBuf {
        self.root.join("note-images")
    }
    pub fn audio(&self) -> PathBuf {
        self.root.join("audio")
    }
    pub fn pomodoro_sound(&self) -> PathBuf {
        self.audio().join("pomodoro_sound.mp3")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn log_file(&self) -> PathBuf {
        self.logs().join("latest.log")
    }
    pub fn rust_log_file(&self) -> PathBuf {
        self.logs().join("rust.log")
    }
    pub fn crash_report(&self) -> PathBuf {
        self.root.join("crash_report.txt")
    }
    pub fn crash_log(&self) -> PathBuf {
        self.root.join("crash_log.txt")
    }
    pub fn tls_identity(&self) -> PathBuf {
        self.root.join("tls-identity.json")
    }
    pub fn initialize_directories(&self) -> std::io::Result<()> {
        for directory in [self.note_images(), self.audio(), self.logs()] {
            if std::fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.is_symlink()) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Application directory cannot be a symbolic link",
                ));
            }
            std::fs::create_dir_all(directory)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/unit/data_paths.rs"]
mod tests;

#[cfg(feature = "runtime")]
pub mod directory;
