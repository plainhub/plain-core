#[cfg(any(feature = "linux-platform", feature = "linux-system"))]
pub mod linux;
#[cfg(feature = "storage-platform")]
pub mod mountinfo;
#[cfg(all(feature = "unix-platform", unix))]
pub mod unix;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod local_socket;

#[cfg(all(target_os = "android", feature = "library"))]
pub mod android;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod download_directory;
pub mod network;

#[cfg(any(feature = "library", feature = "storage-platform"))]
pub mod volumes;
