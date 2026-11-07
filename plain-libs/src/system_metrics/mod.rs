// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

#[macro_use]
mod macros;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd"
))]
mod unix;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd"
))]
use unix::sys;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    windows
)))]
compile_error!("System metrics are unavailable on this target");
#[cfg(feature = "disk-metrics")]
pub use common::disk::Disk;
#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub use common::system::System;
#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub use sys::MINIMUM_CPU_UPDATE_INTERVAL;
#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) use sys::SystemInner;
mod common;

#[cfg(feature = "disk-metrics")]
pub fn disks() -> Vec<Disk> {
    sys::disk::disks()
}
