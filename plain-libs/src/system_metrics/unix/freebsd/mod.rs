#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) mod cpu;
#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) mod system;
pub(crate) mod utils;
#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) use system::{MINIMUM_CPU_UPDATE_INTERVAL, SystemInner};
#[cfg(feature = "disk-metrics")]
pub(crate) mod disk;
