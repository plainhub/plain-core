use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DirEntry {
    path: PathBuf,
    file_type: fs::FileType,
    metadata: fs::Metadata,
}

impl DirEntry {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn file_type(&self) -> fs::FileType {
        self.file_type
    }
    pub fn metadata(&self) -> io::Result<fs::Metadata> {
        Ok(self.metadata.clone())
    }
    pub fn file_name(&self) -> std::ffi::OsString {
        self.path
            .file_name()
            .map(|s| s.to_os_string())
            .unwrap_or_default()
    }
}

pub struct Walk {
    root: PathBuf,
}

impl Walk {
    pub fn new<P: Into<PathBuf>>(root: P) -> Self {
        Self { root: root.into() }
    }
}

impl IntoIterator for Walk {
    type Item = io::Result<DirEntry>;
    type IntoIter = WalkIter;
    fn into_iter(self) -> Self::IntoIter {
        WalkIter {
            stack: vec![self.root],
        }
    }
}

pub struct WalkIter {
    stack: Vec<PathBuf>,
}

impl Iterator for WalkIter {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<Self::Item> {
        let path = self.stack.pop()?;
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => return Some(Err(e)),
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            match fs::read_dir(&path) {
                Ok(rd) => {
                    let mut children: Vec<PathBuf> =
                        rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
                    children.reverse();
                    self.stack.extend(children);
                }
                Err(e) => return Some(Err(e)),
            }
        }
        Some(Ok(DirEntry {
            path,
            file_type: ft,
            metadata: meta,
        }))
    }
}
