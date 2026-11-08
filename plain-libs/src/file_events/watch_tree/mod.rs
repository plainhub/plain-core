// Depth-first traversal adapted from walkdir 2.5.0 (https://github.com/BurntSushi/walkdir), see LICENSE-MIT.

use super::{Error, ErrorKind, Result};
use std::{
    fs::{self, File, FileType, Metadata, ReadDir},
    io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

const MAX_OPEN_DIRECTORIES: usize = 10;

pub(super) struct WatchTree {
    root: Option<PathBuf>,
    stack: Vec<Directory>,
    oldest_open: usize,
}

struct Directory {
    path: PathBuf,
    entries: Entries,
}

enum Entries {
    Open(ReadDir),
    Buffered(std::vec::IntoIter<Result<Entry>>),
    Unreadable(Option<io::Error>),
}

pub(super) struct Entry {
    pub(super) path: PathBuf,
    kind: FileType,
}

impl Entry {
    #[cfg(any(target_os = "linux", target_os = "android", test))]
    pub(super) fn metadata(&self) -> io::Result<Metadata> {
        if self.kind.is_symlink() {
            fs::metadata(&self.path)
        } else {
            fs::symlink_metadata(&self.path)
        }
    }
}

impl WatchTree {
    pub(super) fn new(root: PathBuf) -> Self {
        Self {
            root: Some(root),
            stack: Vec::new(),
            oldest_open: 0,
        }
    }

    fn enter(&mut self, entry: Entry) -> Result<Entry> {
        let directory = if entry.kind.is_symlink() {
            self.follow_directory(&entry.path)?
        } else {
            entry.kind.is_dir()
        };
        if directory {
            if self.stack.len() - self.oldest_open == MAX_OPEN_DIRECTORIES {
                let entries = &mut self.stack[self.oldest_open].entries;
                *entries = Entries::Buffered(entries.by_ref().collect::<Vec<_>>().into_iter());
                self.oldest_open += 1;
            }
            self.stack.push(Directory {
                entries: match fs::read_dir(&entry.path) {
                    Ok(entries) => Entries::Open(entries),
                    Err(error) => Entries::Unreadable(Some(error)),
                },
                path: entry.path.clone(),
            });
        }
        Ok(entry)
    }
    #[cold]
    fn follow_directory(&self, path: &Path) -> Result<bool> {
        if !fs::metadata(path)?.is_dir() {
            return Ok(false);
        }
        let child = File::open(path)?;
        let identity = file_identity(&child.metadata()?);
        for ancestor in self.stack.iter().rev() {
            if identity == file_identity(&File::open(&ancestor.path)?.metadata()?) {
                return Err(Error::new(ErrorKind::Generic(format!(
                    "File system loop found: {} points to an ancestor {}",
                    path.display(),
                    ancestor.path.display()
                ))));
            }
        }
        Ok(true)
    }
}

impl Iterator for WatchTree {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(path) = self.root.take() {
            let entry = fs::symlink_metadata(&path)
                .map(|metadata| Entry {
                    path,
                    kind: metadata.file_type(),
                })
                .map_err(Error::io);
            return Some(entry.and_then(|entry| self.enter(entry)));
        }
        while let Some(directory) = self.stack.last_mut() {
            if let Some(entry) = directory.entries.next() {
                return Some(entry.and_then(|entry| self.enter(entry)));
            }
            self.stack.pop();
            self.oldest_open = self.oldest_open.min(self.stack.len());
        }
        None
    }
}

impl Iterator for Entries {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Open(entries) => entries.next().map(|result| {
                let entry = result?;
                Ok(Entry {
                    kind: entry.file_type()?,
                    path: entry.path(),
                })
            }),
            Self::Buffered(entries) => entries.next(),
            Self::Unreadable(error) => error.take().map(|error| Err(Error::io(error))),
        }
    }
}

fn file_identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/file_events/watch_tree.rs"]
mod tests;
