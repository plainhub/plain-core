#[cfg(feature = "windows-video")]
pub mod media_foundation;
#[cfg(any(feature = "disk-metrics", feature = "system-metrics"))]
pub(crate) mod metrics;
