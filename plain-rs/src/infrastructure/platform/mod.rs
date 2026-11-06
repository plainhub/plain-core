#[cfg(any(feature = "linux-platform", feature = "linux-system"))]
pub mod linux;

#[cfg(all(target_os = "android", feature = "library"))]
pub mod android;
pub mod network;

#[cfg(feature = "runtime")]
pub mod system;

#[cfg(feature = "runtime")]
pub mod disks;

#[cfg(feature = "runtime")]
pub mod image_catalog;

#[cfg(feature = "runtime")]
pub mod startup;

#[cfg(feature = "runtime")]
pub mod media_cache;

#[cfg(feature = "runtime")]
pub mod discovery;

#[cfg(feature = "runtime")]
pub mod version;

#[cfg(all(feature = "runtime", any(target_os = "macos", target_os = "ios")))]
#[path = "apple/video.rs"]
pub mod video;

#[cfg(all(feature = "runtime", target_os = "windows"))]
#[path = "windows/video.rs"]
pub mod video;
