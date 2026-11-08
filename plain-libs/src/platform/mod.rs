#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod apple_device;
pub mod battery;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod download_directory;
#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(feature = "watch", feature = "native-watch")
))]
pub(crate) mod inotify;
#[cfg(windows)]
pub mod known_folders;
#[cfg(feature = "linux-platform")]
pub mod linux;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod local_socket;
#[cfg(feature = "storage-platform")]
pub mod mountinfo;
pub mod network;
#[cfg(all(feature = "unix-platform", unix))]
pub mod unix;

#[cfg(feature = "volumes")]
pub mod volumes;

#[cfg(feature = "peg")]
pub(crate) mod stack_space;

pub(crate) mod rename;
