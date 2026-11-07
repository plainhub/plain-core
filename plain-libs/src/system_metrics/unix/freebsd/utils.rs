// Adapted from sysinfo 0.32.1; see SOURCE.json and LICENSE.

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
#[inline]
pub unsafe fn init_mib(name: &[u8], mib: &mut [libc::c_int]) {
    let mut len = mib.len();
    libc::sysctlnametomib(name.as_ptr() as _, mib.as_mut_ptr(), &mut len);
}

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) fn boot_time() -> u64 {
    let mut boot_time = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut len = std::mem::size_of::<libc::timeval>();
    let mut mib: [libc::c_int; 2] = [libc::CTL_KERN, libc::KERN_BOOTTIME];
    unsafe {
        if libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as _,
            &mut boot_time as *mut libc::timeval as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) < 0
        {
            0
        } else {
            boot_time.tv_sec as _
        }
    }
}

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) unsafe fn get_sys_value<T: Sized>(mib: &[libc::c_int], value: &mut T) -> bool {
    let mut len = std::mem::size_of::<T>() as libc::size_t;
    libc::sysctl(
        mib.as_ptr(),
        mib.len() as _,
        value as *mut _ as *mut _,
        &mut len,
        std::ptr::null_mut(),
        0,
    ) == 0
}

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) unsafe fn get_sys_value_array<T: Sized>(mib: &[libc::c_int], value: &mut [T]) -> bool {
    let mut len = std::mem::size_of_val(value) as libc::size_t;
    libc::sysctl(
        mib.as_ptr(),
        mib.len() as _,
        value.as_mut_ptr() as *mut _,
        &mut len as *mut _,
        std::ptr::null_mut(),
        0,
    ) == 0
}

#[cfg(any(
    feature = "disk-metrics",
    all(feature = "system-metrics", not(target_os = "ios"))
))]
pub(crate) fn c_buf_to_utf8_string(buf: &[libc::c_char]) -> Option<String> {
    let bytes = buf
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte as u8)
        .collect();
    String::from_utf8(bytes).ok()
}

#[cfg(all(feature = "system-metrics", not(target_os = "ios")))]
pub(crate) unsafe fn get_sys_value_by_name<T: Sized>(name: &[u8], value: &mut T) -> bool {
    let mut len = std::mem::size_of::<T>() as libc::size_t;
    let original = len;

    libc::sysctlbyname(
        name.as_ptr() as *const libc::c_char,
        value as *mut _ as *mut _,
        &mut len,
        std::ptr::null_mut(),
        0,
    ) == 0
        && original == len
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/system_metrics/unix/freebsd/utils.rs"]
mod tests;
