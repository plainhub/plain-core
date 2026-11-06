
    use super::*;
    use crate::ffi;
    use crate::runtime::Sel;
    use crate::test_utils;
    use crate::{msg_send, sel};
    use alloc::string::ToString;
    use core::ffi::c_void;
    use core::panic::{RefUnwindSafe, UnwindSafe};

    #[test]
    fn test_verify_message() {
        let cls = test_utils::custom_class();

        assert!(cls.verify_sel::<(), u32>(sel!(foo)).is_ok());
        assert!(cls.verify_sel::<(u32,), ()>(sel!(setFoo:)).is_ok());

        let metaclass = cls.metaclass();
        metaclass
            .verify_sel::<(i32, i32), i32>(sel!(addNumber:toNumber:))
            .unwrap();
    }

    #[test]
    fn test_verify_message_errors() {
        let cls = test_utils::custom_class();

        // Unimplemented selector (missing colon)
        let err = cls.verify_sel::<(), ()>(sel!(setFoo)).unwrap_err();
        assert_eq!(err.to_string(), "method not found");

        // Incorrect return type
        let err = cls.verify_sel::<(u32,), u64>(sel!(setFoo:)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "expected return to have type code 'v', but found 'Q'"
        );

        // Too many arguments
        let err = cls.verify_sel::<(u32, i8), ()>(sel!(setFoo:)).unwrap_err();
        assert_eq!(err.to_string(), "expected 1 arguments, but 2 were given");

        // Too few arguments
        let err = cls.verify_sel::<(), ()>(sel!(setFoo:)).unwrap_err();
        assert_eq!(err.to_string(), "expected 1 arguments, but 0 were given");

        // Incorrect argument type
        let err = cls.verify_sel::<(Sel,), ()>(sel!(setFoo:)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "expected argument at index 0 to have type code 'I', but found ':'"
        );

        // <https://github.com/madsmtm/objc2/issues/566>
        let res = cls.verify_sel::<(), ffi::NSUInteger>(sel!(getNSInteger));
        let expected = if cfg!(any(target_vendor = "apple", any())) {
            Ok(())
        } else if cfg!(target_pointer_width = "64") {
            Err("expected return to have type code 'q', but found 'Q'".to_string())
        } else {
            Err("expected return to have type code 'i', but found 'I'".to_string())
        };
        assert_eq!(res.map_err(|e| e.to_string()), expected);

        // Metaclass
        let metaclass = cls.metaclass();
        let err = metaclass
            .verify_sel::<(i32, i32, i32), i32>(sel!(addNumber:toNumber:))
            .unwrap_err();
        assert_eq!(err.to_string(), "expected 2 arguments, but 3 were given");
    }

    #[test]
    #[cfg(all(debug_assertions, not(any())))]
    #[should_panic = "invalid message send to -[CustomObject foo]: expected return to have type code 'I', but found '^i'"]
    fn test_send_message_verified() {
        let obj = test_utils::custom_object();
        let _: *const i32 = unsafe { msg_send![&obj, foo] };
    }

    #[test]
    #[cfg(all(debug_assertions, not(any())))]
    #[should_panic = "invalid message send to +[CustomObject abcDef]: method not found"]
    fn test_send_message_verified_to_class() {
        let cls = test_utils::custom_class();
        let _: i32 = unsafe { msg_send![cls, abcDef] };
    }

    #[test]
    fn test_marker_traits() {
        fn assert_marker_traits<T: Send + Sync + UnwindSafe + RefUnwindSafe + Unpin>() {}
        assert_marker_traits::<VerificationError>();
    }

    #[test]
    fn test_get_reference() {
        let obj = test_utils::custom_object();
        let _: () = unsafe { msg_send![&obj, setFoo: 42u32] };

        let res: &u32 = unsafe { msg_send![&obj, fooReference] };
        assert_eq!(*res, 42);
        let res: *const u32 = unsafe { msg_send![&obj, fooReference] };
        assert_eq!(unsafe { *res }, 42);
        let res: *mut u32 = unsafe { msg_send![&obj, fooReference] };
        assert_eq!(unsafe { *res }, 42);
    }

    #[test]
    #[cfg_attr(
        all(
            debug_assertions,
            not(feature = "disable-encoding-assertions"),
            not(feature = "relax-void-encoding")
        ),
        should_panic = "invalid message send to -[CustomObject fooReference]: expected return to have type code '^I', but found '^v'"
    )]
    fn test_get_reference_void() {
        let obj = test_utils::custom_object();
        let _: () = unsafe { msg_send![&obj, setFoo: 42u32] };

        let res: *mut c_void = unsafe { msg_send![&obj, fooReference] };
        let res: *mut u32 = res.cast();
        assert_eq!(unsafe { *res }, 42);
    }

    #[test]
    #[cfg(all(debug_assertions, not(any())))]
    #[should_panic = "invalid message send to -[CustomObject foo]: expected return to have type code 'I', but found '^v'"]
    fn test_get_integer_void() {
        let obj = test_utils::custom_object();
        let _: *mut c_void = unsafe { msg_send![&obj, foo] };
    }

