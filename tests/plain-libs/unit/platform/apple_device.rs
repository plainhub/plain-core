use super::*;

#[test]
fn apple_cpu_percent_preserves_idle_failure_and_counter_wrap_semantics() {
    assert_eq!(cpu_usage(None, Some([0; 4])), 0.0);
    assert_eq!(cpu_usage(Some([0; 4]), None), 0.0);
    assert_eq!(cpu_usage(Some([1; 4]), Some([1; 4])), 0.0);
    assert_eq!(cpu_usage(Some([0; 4]), Some([10, 10, 60, 20])), 40.0);
    assert_eq!(cpu_usage(Some([0; 4]), Some([0, 0, 100, 0])), 0.0);
    assert_eq!(
        cpu_usage(Some([u32::MAX - 4, 0, u32::MAX - 4, 0]), Some([5, 0, 5, 0])),
        50.0
    );
}

#[test]
fn apple_system_primitives_return_real_native_values_without_host_requests() {
    assert!(cpu_ticks().is_some());
    assert!(total_memory().unwrap() > 0);
    assert!(!kernel_version().unwrap().is_empty());
}

#[test]
fn native_strings_preserve_empty_unicode_and_long_names() {
    unsafe extern "C" {
        fn CFStringCreateWithBytes(
            allocator: *const std::ffi::c_void,
            bytes: *const u8,
            length: isize,
            encoding: u32,
            external: u8,
        ) -> *const std::ffi::c_void;
        fn CFRelease(value: *const std::ffi::c_void);
    }
    for text in [String::new(), "Disk".into(), "磁盘 😀".repeat(128)] {
        let value = unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                text.as_ptr(),
                text.len() as isize,
                0x08000100,
                0,
            )
        };
        assert!(!value.is_null());
        let decoded = unsafe { cf_string(value) };
        unsafe { CFRelease(value) };
        assert_eq!(decoded, Some(text));
    }
}

#[test]
fn owned_core_foundation_reference_releases_exactly_one_native_reference() {
    unsafe extern "C" {
        fn CFArrayCreateMutable(
            allocator: *const std::ffi::c_void,
            capacity: isize,
            callbacks: *const std::ffi::c_void,
        ) -> *const std::ffi::c_void;
        fn CFRetain(value: *const std::ffi::c_void) -> *const std::ffi::c_void;
        fn CFGetRetainCount(value: *const std::ffi::c_void) -> isize;
    }
    let raw = unsafe { CFArrayCreateMutable(std::ptr::null(), 0, std::ptr::null()) };
    let owned = unsafe { OwnedCF::from_owned(raw) }.unwrap();
    let observer = unsafe { CFRetain(raw) };
    let before = unsafe { CFGetRetainCount(observer) };
    assert_eq!(owned.as_ptr().cast_const(), raw);
    drop(owned);
    let after = unsafe { CFGetRetainCount(observer) };
    unsafe {
        CFRelease(observer);
    }
    assert_eq!(after, before - 1);
}
