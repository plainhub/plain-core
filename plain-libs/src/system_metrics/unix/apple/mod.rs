// Take a look at the license at the top of the repository in the LICENSE file.

#[cfg(feature = "disk-metrics")]
mod ffi;

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub mod cpu;

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub mod system;

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) use self::system::SystemInner;

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub use self::system::MINIMUM_CPU_UPDATE_INTERVAL;

#[cfg(feature = "disk-metrics")]
pub mod disk;
