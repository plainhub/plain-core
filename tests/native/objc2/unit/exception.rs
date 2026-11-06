
    use alloc::format;
    use alloc::string::ToString;
    use core::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;

    use super::*;
    use crate::msg_send;

    #[test]
    fn test_catch() {
        let mut s = "Hello".to_string();
        let result = catch(move || {
            s.push_str(", World!");
            s
        });
        assert_eq!(result.unwrap(), "Hello, World!");
    }

    #[test]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        ignore = "`NULL` exceptions are invalid on 32-bit / w. fragile runtime"
    )]
    fn test_catch_null() {
        let s = "Hello".to_string();
        let result = catch(move || {
            if !s.is_empty() {
                unsafe { ffi::objc_exception_throw(ptr::null_mut()) }
            }
            s.len()
        });
        assert!(result.unwrap_err().is_none());
    }

    #[test]
    #[cfg_attr(any(), ignore = "Panics inside `catch` when catch-all is enabled")]
    fn test_catch_unknown_selector() {
        let obj = AssertUnwindSafe(NSObject::new());
        let ptr = Retained::as_ptr(&obj);
        let result = catch(|| {
            let _: Retained<NSObject> = unsafe { msg_send![&*obj, copy] };
        });
        let err = result.unwrap_err().unwrap();

        assert_eq!(
            format!("{err}"),
            format!("-[NSObject copyWithZone:]: unrecognized selector sent to instance {ptr:?}"),
        );
    }

    #[test]
    fn test_throw_catch_object() {
        let obj = NSObject::new();
        // TODO: Investigate why this is required on GNUStep!
        let _obj2 = obj.clone();
        let obj: Retained<Exception> = unsafe { Retained::cast_unchecked(obj) };
        let ptr: *const Exception = &*obj;

        let result = catch(|| throw(obj));
        let obj = result.unwrap_err().unwrap();

        assert_eq!(format!("{obj:?}"), format!("exception <NSObject: {ptr:p}>"));

        assert!(ptr::eq(&*obj, ptr));
    }

    #[test]
    #[ignore = "currently aborts"]
    fn throw_catch_unwind() {
        let obj = NSObject::new();
        let obj: Retained<Exception> = unsafe { Retained::cast_unchecked(obj) };

        let result = catch_unwind(|| throw(obj));
        let _ = result.unwrap_err();
    }

    #[test]
    #[should_panic = "test"]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86", panic = "unwind"),
        ignore = "panic won't start on 32-bit / w. fragile runtime, it'll just abort, since the runtime uses setjmp/longjump unwinding"
    )]
    fn does_not_catch_panic() {
        let _ = catch(|| panic!("test"));
    }

