pub use error::{Error, ErrorKind, Result};
pub use event::{Event, EventKind};
pub use recursive_mode::RecursiveMode;
use std::path::Path;
pub(crate) use std::sync::mpsc::{Receiver, Sender, channel as unbounded};
mod error;
pub mod event;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) mod inotify;
mod recursive_mode;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub type RecommendedWatcher = inotify::INotifyWatcher;
#[cfg(target_os = "macos")]
pub(crate) mod fsevent;
#[cfg(target_os = "macos")]
mod macos_ffi;
#[cfg(target_os = "macos")]
pub type RecommendedWatcher = fsevent::FsEventWatcher;
#[cfg(windows)]
pub(crate) mod windows;
#[cfg(windows)]
pub type RecommendedWatcher = windows::ReadDirectoryChangesWatcher;
#[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "ios"
))]
pub(crate) mod kqueue;
#[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "ios"
))]
pub type RecommendedWatcher = kqueue::KqueueWatcher;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows,
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly",
    target_os = "ios"
)))]
compile_error!("No native filesystem watcher backend is available for this target");

pub trait EventHandler: Send + 'static {
    fn handle_event(&mut self, event: Result<Event>);
}
impl<F: FnMut(Result<Event>) + Send + 'static> EventHandler for F {
    fn handle_event(&mut self, event: Result<Event>) {
        self(event);
    }
}
impl EventHandler for Sender<Result<Event>> {
    fn handle_event(&mut self, event: Result<Event>) {
        let _ = self.send(event);
    }
}
pub trait Watcher {
    fn new<F: EventHandler>(event_handler: F) -> Result<Self>
    where
        Self: Sized;
    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> Result<()>;
    fn unwatch(&mut self, path: &Path) -> Result<()>;
}
pub fn recommended_watcher<F: EventHandler>(event_handler: F) -> Result<RecommendedWatcher> {
    RecommendedWatcher::new(event_handler)
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/file_events/lib/tests.rs"]
mod tests;

#[cfg(any(windows, test))]
mod windows_records;
