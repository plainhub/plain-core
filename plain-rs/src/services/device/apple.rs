unsafe extern "C" {
    fn mach_host_self() -> u32;
    static mach_task_self_: u32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
}

pub(super) fn cpu_ticks() -> Option<[u32; 4]> {
    let port = unsafe { mach_host_self() };
    let mut ticks = [0u32; 4];
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    let result = unsafe {
        libc::host_statistics(
            port,
            libc::HOST_CPU_LOAD_INFO,
            ticks.as_mut_ptr().cast(),
            &mut count,
        )
    };
    unsafe {
        mach_port_deallocate(mach_task_self_, port);
    }
    (result == 0 && count == libc::HOST_CPU_LOAD_INFO_COUNT).then_some(ticks)
}

pub(super) fn cpu_usage(previous: Option<[u32; 4]>, current: Option<[u32; 4]>) -> f64 {
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

pub(super) fn total_memory() -> Result<i64, String> {
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
    Ok(bytes.min(i64::MAX as u64) as i64)
}

pub(super) fn kernel_version() -> Result<String, String> {
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
#[path = "../../../tests/unit/services/device/apple.rs"]
mod tests;
