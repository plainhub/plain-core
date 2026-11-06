
    use core::ptr;

    use super::*;
    use crate::msg_send;
    use crate::rc::{Allocated, Retained};
    use crate::runtime::NSObject;
    use crate::test_utils;

    #[allow(unused)]
    fn test_different_receivers(obj: &mut AnyObject) {
        unsafe {
            let x = &mut *obj;
            let _: () = msg_send![x, mutable1];
            // `x` is consumed by the above, so this won't work:
            // let _: () = msg_send![x, mutable2];

            // It is only possible if we reborrow:
            let _: () = msg_send![&mut *obj, mutable1];
            let _: () = msg_send![&mut *obj, mutable2];

            // Test NonNull
            let obj = NonNull::from(obj);
            let _: () = msg_send![obj, mutable1];
            let _: () = msg_send![obj, mutable2];

            // And test raw pointers
            let obj: *mut AnyObject = obj.as_ptr();
            let _: () = msg_send![obj, mutable1];
            let _: () = msg_send![obj, mutable2];
        }
    }

    #[test]
    fn test_send_message() {
        let obj = test_utils::custom_object();
        let _: () = unsafe { msg_send![&obj, setFoo: 4u32] };
        let result: u32 = unsafe { msg_send![&obj, foo] };
        assert_eq!(result, 4);
    }

    #[test]
    fn test_send_message_stret() {
        let obj = test_utils::custom_object();
        let result: test_utils::CustomStruct = unsafe { msg_send![&obj, customStruct] };
        let expected = test_utils::CustomStruct {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        };
        assert_eq!(result, expected);
    }

    #[test]
    #[cfg_attr(debug_assertions, should_panic = "messsaging description to nil")]
    fn test_send_message_nil() {
        let nil: *mut NSObject = ::core::ptr::null_mut();

        // This result should not be relied on
        let result: Option<Retained<NSObject>> = unsafe { msg_send![nil, description] };
        assert!(result.is_none());

        // This result should not be relied on
        let result: usize = unsafe { msg_send![nil, hash] };
        assert_eq!(result, 0);

        // This result should not be relied on
        #[cfg(target_pointer_width = "16")]
        let result: f32 = 0.0;
        #[cfg(target_pointer_width = "32")]
        let result: f32 = unsafe { msg_send![nil, floatValue] };
        #[cfg(target_pointer_width = "64")]
        let result: f64 = unsafe { msg_send![nil, doubleValue] };
        assert_eq!(result, 0.0);

        // This result should not be relied on
        let result: Option<Retained<NSObject>> =
            unsafe { msg_send![nil, multiple: 1u32, arguments: 2i8] };
        assert!(result.is_none());

        // This result should not be relied on
        let obj = unsafe { Allocated::new(ptr::null_mut()) };
        let result: Option<Retained<NSObject>> = unsafe { msg_send![obj, init] };
        assert!(result.is_none());
    }

    #[test]
    fn test_send_message_super() {
        let obj = test_utils::custom_subclass_object();
        let superclass = test_utils::custom_class();
        unsafe {
            let _: () = msg_send![&obj, setFoo: 4u32];
            let foo: u32 = msg_send![super(&obj, superclass), foo];
            assert_eq!(foo, 4);

            // The subclass is overridden to return foo + 2
            let foo: u32 = msg_send![&obj, foo];
            assert_eq!(foo, 6);
        }
    }

    #[test]
    #[cfg_attr(any(), ignore = "GNUStep deadlocks here for some reason")]
    fn test_send_message_class_super() {
        let cls = test_utils::custom_subclass();
        let superclass = test_utils::custom_class();
        unsafe {
            let foo: u32 = msg_send![super(cls, superclass.metaclass()), classFoo];
            assert_eq!(foo, 7);

            // The subclass is overridden to return + 2
            let foo: u32 = msg_send![cls, classFoo];
            assert_eq!(foo, 9);
        }
    }

