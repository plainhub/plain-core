// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

#[cfg(feature = "disk-metrics")]
pub(crate) mod disk;
#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) mod system;
