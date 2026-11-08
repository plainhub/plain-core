// Native stack bounds adapted from stacker 0.1.25 (stacker/SOURCE.json).
thread_local! {
    static LIMIT: Option<usize> = lower_bound();
}

pub(crate) fn remaining_stack() -> Option<usize> {
    let marker = 0u8;
    let position = std::hint::black_box(&marker) as *const u8 as usize;
    LIMIT.with(|limit| limit.map(|limit| position.saturating_sub(limit)))
}

#[cfg(all(
    not(miri),
    any(
        target_os = "linux",
        target_os = "solaris",
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
    )
))]
fn lower_bound() -> Option<usize> {
    #[cfg(any(target_os = "freebsd", target_os = "dragonfly", target_os = "illumos"))]
    use libc::pthread_attr_get_np as get_attr;
    #[cfg(any(
        target_os = "linux",
        target_os = "solaris",
        target_os = "netbsd",
        target_os = "haiku"
    ))]
    use libc::pthread_getattr_np as get_attr;

    unsafe {
        let mut attr = std::mem::MaybeUninit::<libc::pthread_attr_t>::uninit();
        if libc::pthread_attr_init(attr.as_mut_ptr()) != 0 {
            return None;
        }
        let mut address = std::ptr::null_mut();
        let mut size = 0;
        let valid = get_attr(libc::pthread_self(), attr.as_mut_ptr()) == 0
            && libc::pthread_attr_getstack(attr.as_ptr(), &mut address, &mut size) == 0;
        let destroyed = libc::pthread_attr_destroy(attr.as_mut_ptr());
        (valid && destroyed == 0 && size != 0 && !address.is_null()).then_some(address as usize)
    }
}

#[cfg(all(not(miri), target_os = "macos"))]
fn lower_bound() -> Option<usize> {
    let thread = unsafe { libc::pthread_self() };
    let top = unsafe { libc::pthread_get_stackaddr_np(thread) } as usize;
    let size = unsafe { libc::pthread_get_stacksize_np(thread) };
    top.checked_sub(size).filter(|&low| low != 0 && size != 0)
}

#[cfg(all(not(miri), target_os = "openbsd"))]
fn lower_bound() -> Option<usize> {
    unsafe {
        let mut stack = std::mem::MaybeUninit::<libc::stack_t>::uninit();
        if libc::pthread_stackseg_np(libc::pthread_self(), stack.as_mut_ptr()) != 0 {
            return None;
        }
        let stack = stack.assume_init();
        (stack.ss_sp as usize).checked_sub(stack.ss_size)
    }
}

#[cfg(all(not(miri), windows))]
fn lower_bound() -> Option<usize> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadStackLimits(low: *mut usize, high: *mut usize);
        fn SetThreadStackGuarantee(bytes: *mut u32) -> i32;
    }

    let (mut low, mut high) = (0, 0);
    let mut guarantee = 0;
    unsafe {
        GetCurrentThreadStackLimits(&mut low, &mut high);
        if SetThreadStackGuarantee(&mut guarantee) == 0 {
            return None;
        }
    }
    let minimum = if cfg!(target_pointer_width = "32") {
        0x1000
    } else {
        0x2000
    };
    let reserve = (guarantee as usize).max(minimum).checked_add(0x2000)?;
    low.checked_add(reserve)
        .filter(|&limit| low != 0 && limit < high)
}

#[cfg(any(
    miri,
    not(any(
        windows,
        target_os = "linux",
        target_os = "solaris",
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "haiku",
        target_os = "illumos",
        target_os = "macos",
        target_os = "openbsd",
    ))
))]
fn lower_bound() -> Option<usize> {
    None
}
