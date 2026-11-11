// System timezone discovery adapted from iana-time-zone 0.1.65 (SOURCE.json).
#[cfg(target_os = "aix")]
mod aix;
#[cfg(target_os = "freebsd")]
mod freebsd;
#[cfg(target_os = "illumos")]
mod illumos;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
mod linux;
#[cfg(target_os = "netbsd")]
mod netbsd;

#[cfg(target_os = "aix")]
use aix as platform;
#[cfg(target_os = "freebsd")]
use freebsd as platform;
#[cfg(target_os = "illumos")]
use illumos as platform;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
use linux as platform;
#[cfg(target_os = "netbsd")]
use netbsd as platform;

#[cfg(any(
    target_os = "aix",
    target_os = "freebsd",
    target_os = "illumos",
    all(target_os = "linux", not(target_env = "ohos")),
    target_os = "netbsd"
))]
enum GetTimezoneError {
    Io,
    #[cfg(any(target_os = "illumos", target_os = "linux", target_os = "netbsd"))]
    FailedParsingString,
    #[cfg(any(target_os = "aix", target_os = "linux"))]
    OsError,
}

#[cfg(any(
    target_os = "aix",
    target_os = "freebsd",
    target_os = "illumos",
    all(target_os = "linux", not(target_env = "ohos")),
    target_os = "netbsd"
))]
impl From<std::io::Error> for GetTimezoneError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

#[cfg(any(
    target_os = "aix",
    target_os = "freebsd",
    target_os = "illumos",
    all(target_os = "linux", not(target_env = "ohos")),
    target_os = "netbsd"
))]
pub(super) fn name() -> Option<String> {
    platform::get_timezone_inner().ok()
}

#[cfg(target_vendor = "apple")]
pub(super) fn name() -> Option<String> {
    use crate::platform::apple_device::{
        CF_STRING_ENCODING_UTF8, CFRelease, CFStringGetCString, CFTimeZoneCopySystem,
        CFTimeZoneGetName, CFTimeZoneResetSystem,
    };
    let mut name = [0; 256];
    // SAFETY: the copied timezone owns its borrowed name until it is released below.
    unsafe {
        CFTimeZoneResetSystem();
        let zone = CFTimeZoneCopySystem();
        if zone.is_null() {
            return None;
        }
        let string = CFTimeZoneGetName(zone);
        let ok = !string.is_null()
            && CFStringGetCString(
                string,
                name.as_mut_ptr(),
                name.len() as _,
                CF_STRING_ENCODING_UTF8,
            ) != 0;
        CFRelease(zone.cast());
        if !ok {
            return None;
        }
    }
    let bytes = name.map(|byte| byte as u8);
    std::ffi::CStr::from_bytes_until_nul(&bytes)
        .ok()?
        .to_str()
        .ok()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

#[cfg(target_os = "android")]
pub(super) fn name() -> Option<String> {
    let mut value = [0u8; libc::PROP_VALUE_MAX as usize];
    // SAFETY: Android writes at most PROP_VALUE_MAX bytes to this buffer.
    let len = unsafe {
        libc::__system_property_get(c"persist.sys.timezone".as_ptr(), value.as_mut_ptr().cast())
    };
    let value = value.get(..usize::try_from(len).ok()?)?;
    std::str::from_utf8(value)
        .ok()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

#[cfg(target_env = "ohos")]
pub(super) fn name() -> Option<String> {
    #[link(name = "begetutil")]
    unsafe extern "C" {
        fn GetParameter(
            key: *const std::ffi::c_char,
            default: *const std::ffi::c_char,
            value: *mut std::ffi::c_char,
            len: u32,
        ) -> i32;
    }
    let mut value = [0u8; 256];
    // SAFETY: GetParameter receives the capacity of the writable buffer.
    let len = unsafe {
        GetParameter(
            c"persist.time.timezone".as_ptr(),
            c"".as_ptr(),
            value.as_mut_ptr().cast(),
            value.len() as u32,
        )
    };
    let value = value.get(..usize::try_from(len).ok()?)?;
    std::str::from_utf8(value)
        .ok()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

#[cfg(not(any(
    target_os = "aix",
    target_os = "android",
    target_os = "freebsd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "netbsd",
    target_vendor = "apple",
    target_env = "ohos"
)))]
pub(super) fn name() -> Option<String> {
    None
}
