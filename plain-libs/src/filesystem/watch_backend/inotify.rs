use crate::filesystem::watch_backend::*;
#[cfg(test)]
use crate::platform::inotify::decode;
use crate::platform::inotify::{self, RawEvent};
use std::{collections::HashMap, io, os::fd::OwnedFd};
use tokio::io::{Interest, unix::AsyncFd};

pub struct Watcher {
    fd: AsyncFd<OwnedFd>,
    directories: HashMap<i32, PathBuf>,
}

impl Watcher {
    pub fn new() -> io::Result<Option<Self>> {
        if !cfg!(target_os = "android") {
            return Ok(None);
        }
        Self::open().map(Some)
    }

    fn open() -> io::Result<Self> {
        let fd = inotify::open()?;
        // SAFETY: The owned descriptor remains open and stable until AsyncFd drops it.
        let fd = unsafe { AsyncFd::register_with_interest(fd, Interest::READABLE) }?;
        Ok(Self {
            fd,
            directories: HashMap::new(),
        })
    }

    pub fn add(&mut self, path: &Path) -> io::Result<()> {
        if self.directories.values().any(|current| current == path) {
            return Ok(());
        }
        let mask = libc::IN_CLOSE_WRITE
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO
            | libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF
            | libc::IN_ONLYDIR
            | libc::IN_DONT_FOLLOW;
        let descriptor = inotify::add(self.fd.get_ref(), path, mask)?;
        self.directories.insert(descriptor, path.to_owned());
        Ok(())
    }

    pub fn moved(&mut self, source: &Path, destination: &Path) {
        for path in self.directories.values_mut() {
            if let Ok(relative) = path.strip_prefix(source) {
                *path = destination.join(relative);
            }
        }
    }

    pub fn remove(&mut self, root: &Path) {
        let selected = self
            .directories
            .iter()
            .filter_map(|(wd, path)| path.starts_with(root).then_some(*wd))
            .collect::<Vec<_>>();
        for wd in selected {
            self.directories.remove(&wd);
            let _ = inotify::remove(self.fd.get_ref(), wd);
        }
    }

    pub async fn read(&self) -> io::Result<Vec<RawEvent>> {
        let mut buffer = vec![0u8; 65536];
        loop {
            let mut ready = self.fd.readable().await?;
            match ready.try_io(|fd| inotify::read(fd.get_ref(), &mut buffer)) {
                Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => continue,
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }

    pub fn change(&mut self, event: RawEvent) -> Option<Change> {
        if event.has(libc::IN_Q_OVERFLOW) {
            return Some(Change {
                path: PathBuf::new(),
                kind: Kind::Refresh,
            });
        }
        let parent = self.directories.get(&event.descriptor)?.clone();
        if event.has(libc::IN_IGNORED) {
            self.directories.remove(&event.descriptor);
            return Some(Change {
                path: parent,
                kind: Kind::Refresh,
            });
        }
        let path = if event.name.is_empty() {
            parent
        } else {
            parent.join(&event.name)
        };
        let kind = if event.has(libc::IN_UNMOUNT | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF) {
            Kind::Refresh
        } else if event.has(libc::IN_MOVED_FROM) {
            Kind::From(event.cookie)
        } else if event.has(libc::IN_MOVED_TO) {
            Kind::To(event.cookie)
        } else if event.has(libc::IN_DELETE) {
            Kind::Removed
        } else if event.has(libc::IN_ISDIR) {
            Kind::Directory
        } else if event.has(libc::IN_CLOSE_WRITE) {
            Kind::Completed
        } else {
            return None;
        };
        Some(Change { path, kind })
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/filesystem/watch_backend/inotify.rs"]
mod tests;
