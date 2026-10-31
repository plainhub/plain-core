use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Completed,
    Removed,
    From(u32),
    To(u32),
    Directory,
    Refresh,
}

#[derive(Clone, Debug)]
pub(super) struct Change {
    pub path: PathBuf,
    pub kind: Kind,
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[path = "watch_backend/inotify.rs"]
mod system;

#[cfg(not(any(target_os = "android", target_os = "linux")))]
mod system {
    use super::*;
    use std::io;
    pub struct Watcher;
    pub struct Event;
    impl Watcher {
        pub fn new() -> io::Result<Option<Self>> {
            Ok(None)
        }
        pub fn add(&mut self, _path: &Path) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }
        pub fn moved(&mut self, _source: &Path, _destination: &Path) {}
        pub fn remove(&mut self, _path: &Path) {}
        pub async fn read(&self) -> io::Result<Vec<Event>> {
            std::future::pending().await
        }
        pub fn change(&mut self, _event: Event) -> Option<Change> {
            None
        }
    }
}

pub(super) use system::Watcher;
