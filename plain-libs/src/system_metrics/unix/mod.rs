#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) mod apple;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) use apple as sys;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) mod linux;
#[cfg(target_os = "android")]
pub(crate) use libc::__errno as libc_errno;
#[cfg(target_os = "linux")]
pub(crate) use libc::__errno_location as libc_errno;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) use linux as sys;
#[cfg(target_os = "freebsd")]
pub(crate) mod freebsd;
#[cfg(target_os = "freebsd")]
pub(crate) use freebsd as sys;
