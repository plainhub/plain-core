use std::ffi::c_void;

#[cfg(any(
    feature = "volumes",
    all(target_os = "macos", feature = "native-watch")
))]
pub(crate) const CF_URL_POSIX_PATH_STYLE: isize = 0;

unsafe extern "C" {
    fn mach_host_self() -> u32;
    static mach_task_self_: u32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    #[cfg(target_os = "macos")]
    pub(crate) fn CFArrayGetCount(array: *const c_void) -> isize;
    #[cfg(target_os = "macos")]
    pub(crate) fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    #[cfg(any(
        feature = "volumes",
        all(
            target_os = "macos",
            any(feature = "native-watch", feature = "disk-metrics")
        )
    ))]
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: *const c_void,
        bytes: *const u8,
        len: isize,
        directory: u8,
    ) -> *const c_void;
    #[cfg(any(
        feature = "volumes",
        all(target_os = "macos", feature = "native-watch")
    ))]
    pub(crate) fn CFURLCopyFileSystemPath(url: *const c_void, style: isize) -> *const c_void;
    fn CFRelease(value: *const std::ffi::c_void);
    fn CFStringGetLength(value: *const std::ffi::c_void) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        value: *const std::ffi::c_void,
        buffer: *mut std::ffi::c_char,
        size: isize,
        encoding: u32,
    ) -> u8;
}

#[cfg(any(
    feature = "volumes",
    all(
        target_os = "macos",
        any(feature = "native-watch", feature = "disk-metrics")
    )
))]
pub(crate) fn cf_file_url(path: &std::path::Path) -> Option<OwnedCF> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = path.as_os_str().as_bytes();
    unsafe {
        OwnedCF::from_owned(CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            0,
        ))
    }
}

pub(crate) struct OwnedCF<T = std::ffi::c_void>(std::ptr::NonNull<T>);
impl<T> OwnedCF<T> {
    // The pointer must transfer one live Create/Copy reference, released exactly once by this owner.
    pub(crate) unsafe fn from_owned(pointer: *const T) -> Option<Self> {
        std::ptr::NonNull::new(pointer.cast_mut()).map(Self)
    }
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.0.as_ptr()
    }
}
impl<T> Drop for OwnedCF<T> {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.0.as_ptr().cast());
        }
    }
}

// The caller supplies a live, non-null CFStringRef for the duration of this copy.
pub(crate) unsafe fn cf_string(value: *const std::ffi::c_void) -> Option<String> {
    const UTF8: u32 = 0x08000100;
    let size = unsafe { CFStringGetMaximumSizeForEncoding(CFStringGetLength(value), UTF8) };
    let capacity = size.checked_add(1)?;
    if capacity <= 0 {
        return None;
    }
    let mut bytes = vec![0u8; capacity as usize];
    if unsafe { CFStringGetCString(value, bytes.as_mut_ptr().cast(), capacity, UTF8) } == 0 {
        return None;
    }
    std::ffi::CStr::from_bytes_until_nul(&bytes)
        .ok()?
        .to_str()
        .ok()
        .map(str::to_owned)
}

pub(crate) struct HostPort(u32);
impl HostPort {
    pub(crate) fn new() -> Self {
        Self(unsafe { mach_host_self() })
    }
    pub(crate) fn get(&self) -> u32 {
        self.0
    }
}
impl Drop for HostPort {
    fn drop(&mut self) {
        unsafe {
            mach_port_deallocate(mach_task_self_, self.0);
        }
    }
}

pub fn cpu_ticks() -> Option<[u32; 4]> {
    let port = HostPort::new();
    let mut ticks = [0u32; 4];
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    let result = unsafe {
        libc::host_statistics(
            port.get(),
            libc::HOST_CPU_LOAD_INFO,
            ticks.as_mut_ptr().cast(),
            &mut count,
        )
    };
    (result == 0 && count == libc::HOST_CPU_LOAD_INFO_COUNT).then_some(ticks)
}

pub fn cpu_usage(previous: Option<[u32; 4]>, current: Option<[u32; 4]>) -> f64 {
    let (Some(previous), Some(current)) = (previous, current) else {
        return 0.0;
    };
    let changes = std::array::from_fn::<_, 4, _>(|index| {
        u64::from(current[index].wrapping_sub(previous[index]))
    });
    let total: u64 = changes.iter().sum();
    if total == 0 {
        return 0.0;
    }
    (total - changes[libc::CPU_STATE_IDLE as usize]) as f64 * 100.0 / total as f64
}

pub fn total_memory() -> Result<i64, String> {
    memory_bytes().map(|bytes| bytes.min(i64::MAX as u64) as i64)
}

pub(crate) fn memory_bytes() -> Result<u64, String> {
    let mut bytes = 0u64;
    let mut length = std::mem::size_of_val(&bytes);
    let result = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut bytes as *mut u64).cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if length != std::mem::size_of_val(&bytes) {
        return Err("Invalid physical memory size".into());
    }
    Ok(bytes)
}

pub(crate) fn sysctl_string(name: &std::ffi::CStr) -> Option<String> {
    let mut length = 0;
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length == 0
    {
        return None;
    }
    let mut bytes = vec![0; length];
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || length > bytes.len()
    {
        return None;
    }
    bytes.truncate(length);
    if let Some(end) = bytes.iter().position(|byte| *byte == 0) {
        bytes.truncate(end);
    }
    String::from_utf8(bytes).ok()
}

pub fn kernel_version() -> Result<String, String> {
    let mut name = std::mem::MaybeUninit::<libc::utsname>::uninit();
    if unsafe { libc::uname(name.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let name = unsafe { name.assume_init() };
    Ok(unsafe { std::ffi::CStr::from_ptr(name.release.as_ptr()) }
        .to_string_lossy()
        .into_owned())
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/platform/apple_device.rs"]
mod tests;
