use plain_rs::services::files::watch_backend::*;
use std::{
    collections::HashMap,
    ffi::{CString, OsString},
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStringExt,
    },
};
use tokio::io::unix::AsyncFd;

pub struct Watcher {
    fd: AsyncFd<OwnedFd>,
    directories: HashMap<i32, PathBuf>,
}

pub struct Event {
    descriptor: i32,
    mask: u32,
    cookie: u32,
    name: OsString,
}

impl Watcher {
    pub fn new() -> io::Result<Option<Self>> {
        if !cfg!(target_os = "android") {
            return Ok(None);
        }
        Self::open().map(Some)
    }

    fn open() -> io::Result<Self> {
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(Self {
            fd: AsyncFd::new(fd)?,
            directories: HashMap::new(),
        })
    }

    pub fn add(&mut self, path: &Path) -> io::Result<()> {
        if self.directories.values().any(|current| current == path) {
            return Ok(());
        }
        let name = CString::new(path.as_os_str().as_encoded_bytes())?;
        let mask = libc::IN_CLOSE_WRITE
            | libc::IN_MOVED_FROM
            | libc::IN_MOVED_TO
            | libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF
            | libc::IN_ONLYDIR
            | libc::IN_DONT_FOLLOW;
        let descriptor =
            unsafe { libc::inotify_add_watch(self.fd.get_ref().as_raw_fd(), name.as_ptr(), mask) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
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
            unsafe {
                libc::inotify_rm_watch(self.fd.get_ref().as_raw_fd(), wd as _);
            }
        }
    }

    pub async fn read(&self) -> io::Result<Vec<Event>> {
        let mut buffer = vec![0u8; 65536];
        loop {
            let mut ready = self.fd.readable().await?;
            let result = ready.try_io(|fd| {
                let count = unsafe {
                    libc::read(
                        fd.get_ref().as_raw_fd(),
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                    )
                };
                if count < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(count as usize)
                }
            });
            let count = match result {
                Ok(Err(error)) if error.kind() == io::ErrorKind::Interrupted => continue,
                Ok(result) => result?,
                Err(_) => continue,
            };
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "filesystem watcher closed",
                ));
            }
            return decode(&buffer[..count]);
        }
    }

    pub fn change(&mut self, event: Event) -> Option<Change> {
        if event.mask & libc::IN_Q_OVERFLOW != 0 {
            return Some(Change {
                path: PathBuf::new(),
                kind: Kind::Refresh,
            });
        }
        let parent = self.directories.get(&event.descriptor)?.clone();
        if event.mask & libc::IN_IGNORED != 0 {
            self.directories.remove(&event.descriptor);
            return Some(Change {
                path: parent,
                kind: Kind::Refresh,
            });
        }
        let path = if event.name.is_empty() {
            parent
        } else {
            parent.join(event.name)
        };
        let kind =
            if event.mask & (libc::IN_UNMOUNT | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF) != 0 {
                Kind::Refresh
            } else if event.mask & libc::IN_MOVED_FROM != 0 {
                Kind::From(event.cookie)
            } else if event.mask & libc::IN_MOVED_TO != 0 {
                Kind::To(event.cookie)
            } else if event.mask & libc::IN_DELETE != 0 {
                Kind::Removed
            } else if event.mask & libc::IN_ISDIR != 0 {
                Kind::Directory
            } else if event.mask & libc::IN_CLOSE_WRITE != 0 {
                Kind::Completed
            } else {
                return None;
            };
        Some(Change { path, kind })
    }
}

fn decode(bytes: &[u8]) -> io::Result<Vec<Event>> {
    let mut events = Vec::new();
    let mut offset = 0;
    let header = std::mem::size_of::<libc::inotify_event>();
    while offset < bytes.len() {
        if bytes.len() - offset < header {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated filesystem event",
            ));
        }
        let raw = unsafe {
            std::ptr::read_unaligned(bytes[offset..].as_ptr().cast::<libc::inotify_event>())
        };
        let end = offset
            .checked_add(header)
            .and_then(|n| n.checked_add(raw.len as usize))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid filesystem event length",
                )
            })?;
        let name = &bytes[offset + header..end];
        let length = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        let name = OsString::from_vec(name[..length].to_vec());
        if !name.is_empty()
            && (Path::new(&name).components().count() != 1
                || Path::new(&name)
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_))))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid filesystem event name",
            ));
        }
        events.push(Event {
            descriptor: raw.wd,
            mask: raw.mask,
            cookie: raw.cookie,
            name,
        });
        offset = end;
    }
    Ok(events)
}

#[cfg(test)]
#[path = "../../../../tests/unit/services/files/watch_backend/inotify.rs"]
mod tests;
