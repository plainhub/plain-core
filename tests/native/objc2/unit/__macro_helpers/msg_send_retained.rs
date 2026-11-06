
    use core::mem::ManuallyDrop;

    use super::*;

    use crate::rc::{autoreleasepool, Allocated, PartialInit, RcTestObject, ThreadTestData};
    use crate::runtime::{AnyObject, NSObject, NSObjectProtocol, NSZone};
    use crate::{class, define_class, extern_methods, msg_send, test_utils, AnyThread};

    #[test]
    fn test_send_message_manuallydrop() {
        let obj = ManuallyDrop::new(test_utils::custom_object());
        unsafe {
            let _: () = msg_send![obj, release];
        };
        // `obj` is consumed, can't use here
    }

    macro_rules! test_error_bool {
        ($expected:expr, $($obj:tt)*) => {
            // Succeeds
            let res: Result<(), Retained<NSObject>> = unsafe {
                msg_send![$($obj)*, boolAndShouldError: false, error: _]
            };
            assert_eq!(res, Ok(()));
            $expected.assert_current();

            // Errors
            let res = autoreleasepool(|_pool| {
                let res: Result<(), Retained<NSObject>> = unsafe {
                    msg_send![$($obj)*, boolAndShouldError: true, error: _]
                };
                let res = res.expect_err("not err");
                $expected.alloc += 1;
                $expected.init += 1;
                $expected.autorelease += 1;
                $expected.retain += 1;
                $expected.assert_current();
                res
            });
            $expected.release += 1;
            $expected.assert_current();

            drop(res);
            $expected.release += 1;
            $expected.drop += 1;
            $expected.assert_current();
        }
    }

    define_class!(
        #[unsafe(super(RcTestObject, NSObject))]
        #[derive(Debug, PartialEq, Eq)]
        struct RcTestObjectSubclass;
    );

    #[cfg_attr(not(test), allow(unused))]
    impl RcTestObjectSubclass {
        fn new() -> Retained<Self> {
            unsafe { msg_send![Self::class(), new] }
        }
    }

    #[test]
    fn test_error_bool() {
        let mut expected = ThreadTestData::current();

        let cls = RcTestObject::class();
        test_error_bool!(expected, cls);

        let obj = RcTestObject::new();
        expected.alloc += 1;
        expected.init += 1;
        test_error_bool!(expected, &obj);

        let obj = RcTestObjectSubclass::new();
        expected.alloc += 1;
        expected.init += 1;
        test_error_bool!(expected, &obj);
        test_error_bool!(expected, super(&obj));
        test_error_bool!(expected, super(&obj, RcTestObjectSubclass::class()));
        test_error_bool!(expected, super(&obj, RcTestObject::class()));
    }

    mod test_trait_disambugated {
        use super::*;

        #[allow(dead_code)]
        trait Abc {
            fn send_message(&self) {}
        }

        impl<T> Abc for T {}

        #[test]
        fn test_macro_still_works() {
            let _: Retained<NSObject> = unsafe { msg_send![NSObject::class(), new] };
        }
    }

    // `new` family

    #[test]
    fn test_new() {
        let mut expected = ThreadTestData::current();
        let cls = RcTestObject::class();

        let _obj: Retained<AnyObject> = unsafe { msg_send![cls, new] };
        let _obj: Option<Retained<AnyObject>> = unsafe { msg_send![cls, new] };
        // This is just a roundabout way of calling `[__RcTestObject new]`.
        let _obj: Retained<AnyObject> = unsafe { msg_send![super(cls, cls.metaclass()), new] };
        let _obj: Option<Retained<AnyObject>> =
            unsafe { msg_send![super(cls, cls.metaclass()), new] };

        // `__RcTestObject` does not override `new`, so this just ends up
        // calling `[[__RcTestObject alloc] init]` as usual.
        let _obj: Retained<RcTestObject> =
            unsafe { msg_send![super(cls, NSObject::class().metaclass()), new] };

        expected.alloc += 5;
        expected.init += 5;
        expected.assert_current();
    }

    #[test]
    fn test_new_not_on_class() {
        let mut expected = ThreadTestData::current();
        let obj = RcTestObject::new();
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        let _obj: Retained<AnyObject> = unsafe { msg_send![&obj, newMethodOnInstance] };
        let _obj: Option<Retained<AnyObject>> = unsafe { msg_send![&obj, newMethodOnInstance] };
        let _obj: Retained<AnyObject> =
            unsafe { msg_send![super(&obj, RcTestObject::class()), newMethodOnInstance] };
        let _obj: Option<Retained<AnyObject>> =
            unsafe { msg_send![super(&obj, RcTestObject::class()), newMethodOnInstance] };
        expected.alloc += 4;
        expected.init += 4;
        expected.assert_current();
    }

    #[test]
    // newScriptingObjectOfClass only available on macOS
    #[cfg_attr(not(all(target_vendor = "apple", target_os = "macos")), ignore)]
    fn test_new_with_args() {
        let mut expected = ThreadTestData::current();

        let object_class = RcTestObject::class();
        let key: Retained<AnyObject> = unsafe { msg_send![class!(NSString), new] };
        let contents_value: *const AnyObject = ptr::null();
        let properties: Retained<AnyObject> = unsafe { msg_send![class!(NSDictionary), new] };

        let _obj: Option<Retained<AnyObject>> = unsafe {
            msg_send![
                NSObject::class(),
                newScriptingObjectOfClass: object_class,
                forValueForKey: &*key,
                withContentsValue: contents_value,
                properties: &*properties,
            ]
        };
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();
    }

    #[test]
    #[should_panic = "failed creating new instance of NSValue"]
    // GNUStep instead returns an invalid instance that panics on accesses
    #[cfg_attr(any(), ignore)]
    fn new_nsvalue_fails() {
        let _val: Retained<AnyObject> = unsafe { msg_send![class!(NSValue), new] };
    }

    #[test]
    #[should_panic = "failed creating new instance using +[__RcTestObject newReturningNull]"]
    fn test_new_with_null() {
        let _obj: Retained<RcTestObject> =
            unsafe { msg_send![RcTestObject::class(), newReturningNull] };
    }

    #[test]
    #[should_panic = "failed creating new instance using +[__RcTestObject newReturningNull]"]
    fn test_super_new_with_null() {
        let _: Retained<RcTestObject> = unsafe {
            msg_send![
                super(RcTestObject::class(), RcTestObject::class().metaclass()),
                newReturningNull
            ]
        };
    }

    #[test]
    #[should_panic = "unexpected NULL returned from -[__RcTestObject newMethodOnInstanceNull]"]
    fn test_new_any_with_null() {
        let obj = RcTestObject::new();
        let _obj: Retained<AnyObject> = unsafe { msg_send![&obj, newMethodOnInstanceNull] };
    }

    #[test]
    #[should_panic = "unexpected NULL returned from -[__RcTestObject newMethodOnInstanceNull]"]
    fn test_super_new_any_with_null() {
        let obj = RcTestObject::new();
        let _obj: Retained<AnyObject> =
            unsafe { msg_send![super(&obj, RcTestObject::class()), newMethodOnInstanceNull] };
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic = "messsaging newMethodOnInstance to nil"
    )]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "unexpected NULL newMethodOnInstance; receiver was NULL"
    )]
    fn test_new_any_with_null_receiver() {
        let obj: *const NSObject = ptr::null();
        let _obj: Retained<AnyObject> = unsafe { msg_send![obj, newMethodOnInstance] };
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic = "messsaging newMethodOnInstance to nil"
    )]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "unexpected NULL newMethodOnInstance; receiver was NULL"
    )]
    fn test_super_new_any_with_null_receiver() {
        let obj: *const RcTestObject = ptr::null();
        let _obj: Retained<AnyObject> = unsafe { msg_send![super(obj), newMethodOnInstance] };
    }

    // `alloc` family

    #[test]
    fn test_alloc() {
        let mut expected = ThreadTestData::current();
        let cls = RcTestObject::class();

        let obj: Allocated<RcTestObject> = unsafe { msg_send![cls, alloc] };
        expected.alloc += 1;
        expected.assert_current();

        drop(obj);
        expected.release += 1;
        // Drop flag ensures uninitialized do not Drop
        // expected.drop += 1;
        expected.assert_current();

        // `+[NSObject alloc]` forwards to `allocWithZone:`, so this still
        // allocates a `__RcTestObject`.
        let _: Allocated<NSObject> =
            unsafe { msg_send![super(cls, NSObject::class().metaclass()), alloc] };
        expected.alloc += 1;
        expected.release += 1;
        // Drop flag ensures uninitialized do not Drop
        // expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    fn test_alloc_with_zone() {
        let mut expected = ThreadTestData::current();
        let cls = RcTestObject::class();
        let zone: *const NSZone = ptr::null();

        let _obj: Allocated<RcTestObject> = unsafe { msg_send![cls, allocWithZone: zone] };
        expected.alloc += 1;
        expected.assert_current();

        let _obj: Allocated<RcTestObject> =
            unsafe { msg_send![super(cls, cls.metaclass()), allocWithZone: zone] };
        expected.alloc += 1;
        expected.assert_current();

        let _obj: Allocated<NSObject> =
            unsafe { msg_send![super(cls, NSObject::class().metaclass()), allocWithZone: zone] };
        expected.assert_current();
    }

    #[test]
    fn test_alloc_with_null() {
        let obj: Allocated<RcTestObject> =
            unsafe { msg_send![RcTestObject::class(), allocReturningNull] };
        assert!(Allocated::as_ptr(&obj).is_null());
    }

    // `init` family

    #[test]
    fn test_init() {
        let mut expected = ThreadTestData::current();

        let _: Retained<RcTestObject> = unsafe { msg_send![RcTestObject::alloc(), init] };
        expected.alloc += 1;
        expected.init += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        let obj = RcTestObject::alloc().set_ivars(());
        let _: Retained<RcTestObject> = unsafe { msg_send![super(obj), init] };
        expected.alloc += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        // Check allocation error before init
        let obj = RcTestObject::alloc();
        expected.alloc += 1;
        assert!(!Allocated::as_ptr(&obj).is_null());
        let _: Retained<RcTestObject> = unsafe { msg_send![obj, init] };
        expected.init += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    #[should_panic = "failed initializing object with -initReturningNull"]
    fn test_init_with_null() {
        let obj: Allocated<RcTestObject> = unsafe { msg_send![RcTestObject::class(), alloc] };
        let _obj: Retained<RcTestObject> = unsafe { msg_send![obj, initReturningNull] };
    }

    #[test]
    #[cfg_attr(debug_assertions, should_panic = "messsaging init to nil")]
    #[cfg_attr(not(debug_assertions), ignore = "failed allocating object")]
    fn test_init_with_null_receiver() {
        let obj: Allocated<RcTestObject> =
            unsafe { msg_send![RcTestObject::class(), allocReturningNull] };
        let _obj: Retained<RcTestObject> = unsafe { msg_send![obj, init] };
    }

    #[test]
    #[should_panic = "tried to initialize ivars after they were already initialized"]
    #[cfg_attr(not(debug_assertions), ignore = "only checked with debug assertions")]
    fn test_super_init_not_initialized() {
        let obj = RcTestObject::alloc().set_ivars(());
        let _: Retained<RcTestObject> =
            unsafe { msg_send![super(obj, RcTestObject::class()), init] };
    }

    #[test]
    #[should_panic = "tried to finalize an already finalized object"]
    #[cfg_attr(not(debug_assertions), ignore = "only checked with debug assertions")]
    fn test_super_init_not_finalized() {
        let obj = unsafe { PartialInit::new(Allocated::into_ptr(RcTestObject::alloc())) };
        let _: Retained<RcTestObject> =
            unsafe { msg_send![super(obj, RcTestObject::class()), init] };
    }

    // `copy` family

    #[test]
    fn test_copy() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let _: Retained<RcTestObject> = unsafe { msg_send![&obj, copy] };
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        // `+[NSObject copy]` forwards to `copyWithZone:`, so this still
        // creates a `__RcTestObject`.
        let _: Retained<NSObject> = unsafe { msg_send![super(&obj), copy] };
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    #[should_panic = "failed copying object"]
    fn test_copy_with_null() {
        let obj = RcTestObject::new();
        let _obj: Retained<RcTestObject> = unsafe { msg_send![&obj, copyReturningNull] };
    }

    #[test]
    #[should_panic = "failed copying object"]
    fn test_super_copy_with_null() {
        let obj = RcTestObject::new();
        let _obj: Retained<RcTestObject> =
            unsafe { msg_send![super(&obj, RcTestObject::class()), copyReturningNull] };
    }

    // `mutableCopy` family

    #[test]
    fn test_mutable_copy() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let _: Retained<RcTestObject> = unsafe { msg_send![&obj, mutableCopy] };
        expected.mutable_copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        // `+[NSObject mutableCopy]` forwards to `mutableCopyWithZone:`, so
        // this still creates a `__RcTestObject`.
        let _: Retained<NSObject> = unsafe { msg_send![super(&obj), mutableCopy] };
        expected.mutable_copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    // No method family

    #[test]
    fn test_normal() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let _: Retained<RcTestObject> = unsafe { msg_send![&obj, self] };
        expected.retain += 1;
        expected.release += 1;
        expected.assert_current();

        let _: Retained<RcTestObject> = unsafe { msg_send![super(&obj), self] };
        expected.retain += 1;
        expected.release += 1;
        expected.assert_current();

        let _: Option<Retained<RcTestObject>> = unsafe { msg_send![&obj, description] };
        expected.assert_current();

        let _: Option<Retained<RcTestObject>> = unsafe { msg_send![super(&obj), description] };
        expected.assert_current();
    }

    #[test]
    #[should_panic = "unexpected NULL returned from -[__RcTestObject methodReturningNull]"]
    fn test_normal_with_null() {
        let obj = RcTestObject::new();
        let _obj: Retained<RcTestObject> = unsafe { msg_send![&obj, methodReturningNull] };
    }

    #[test]
    #[should_panic = "unexpected NULL returned from -[__RcTestObject aMethod:]"]
    fn test_normal_with_param_and_null() {
        let obj = RcTestObject::new();
        let _obj: Retained<RcTestObject> = unsafe { msg_send![&obj, aMethod: false] };
    }

    #[test]
    #[cfg_attr(debug_assertions, should_panic = "messsaging description to nil")]
    #[cfg_attr(
        not(debug_assertions),
        ignore = "unexpected NULL description; receiver was NULL"
    )]
    fn test_normal_with_null_receiver() {
        let obj: *const NSObject = ptr::null();
        let _obj: Retained<AnyObject> = unsafe { msg_send![obj, description] };
    }

    /// This is imperfect, but will do for now.
    const fn autorelease_skipped(self_declared: bool) -> bool {
        if cfg!(any()) {
            // GNUStep does the optimization a different way, so it isn't
            // optimization-dependent.
            true
        } else if cfg!(all(target_arch = "arm", panic = "unwind")) {
            // 32-bit ARM unwinding sometimes interferes with the optimization
            false
        } else if self_declared {
            // FIXME: Autorelease_return is not currently tail-called, so the
            // optimization doesn't work on define_class! functions.
            false
        } else if cfg!(any()) {
            // FIXME: `catch-all` is inserted before we get a chance to retain.
            false
        } else if cfg!(debug_assertions) {
            // `debug_assertions` ~proxy for if optimizations are off.
            false
        } else {
            true
        }
    }

    macro_rules! test_error_retained {
        ($expected:expr, $if_autorelease_not_skipped:expr, $sel:ident, $($obj:tt)*) => {
            // Succeeds
            let res = autoreleasepool(|_pool| {
                let res: Result<Retained<RcTestObject>, Retained<NSObject>> = unsafe {
                    msg_send![$($obj)*, $sel: false, error: _]
                };
                let res = res.expect("not ok");
                $expected.alloc += 1;
                $expected.init += 1;
                $expected.autorelease += $if_autorelease_not_skipped;
                $expected.retain += $if_autorelease_not_skipped;
                $expected.assert_current();
                res
            });
            $expected.release += $if_autorelease_not_skipped;
            $expected.assert_current();

            drop(res);
            $expected.release += 1;
            $expected.drop += 1;
            $expected.assert_current();

            // Errors
            let res = autoreleasepool(|_pool| {
                let res: Result<Retained<RcTestObject>, Retained<NSObject>> = unsafe {
                    msg_send![$($obj)*, $sel: true, error: _]
                };
                $expected.alloc += 1;
                $expected.init += 1;
                $expected.autorelease += 1;
                $expected.retain += 1;
                $expected.assert_current();
                res.expect_err("not err")
            });
            $expected.release += 1;
            $expected.assert_current();

            drop(res);
            $expected.release += 1;
            $expected.drop += 1;
            $expected.assert_current();
        }
    }

    #[test]
    fn test_error_retained() {
        let mut expected = ThreadTestData::current();

        let cls = RcTestObject::class();
        test_error_retained!(
            expected,
            if autorelease_skipped(true) { 0 } else { 1 },
            idAndShouldError,
            cls
        );
        test_error_retained!(expected, 0, newAndShouldError, cls);

        let obj = RcTestObject::new();
        expected.alloc += 1;
        expected.init += 1;
        test_error_retained!(
            expected,
            if autorelease_skipped(true) { 0 } else { 1 },
            idAndShouldError,
            &obj
        );

        expected.alloc -= 1;
        expected.release -= 1;
        test_error_retained!(expected, 0, initAndShouldError, {
            expected.alloc += 1;
            expected.release += 1;
            // Drop flag ensures newly allocated objects do not drop
            // expected.drop += 1;
            RcTestObject::alloc()
        });
    }

    #[test]
    fn test_method_with_param() {
        let mut expected = ThreadTestData::current();

        let obj = RcTestObject::new();
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        let res: Option<Retained<RcTestObject>> = unsafe { msg_send![&obj, aMethod: false] };
        assert!(res.is_none());
        expected.assert_current();

        let _res = autoreleasepool(|_pool| {
            let res: Option<Retained<RcTestObject>> = unsafe { msg_send![&obj, aMethod: true] };
            assert!(res.is_some());
            expected.alloc += 1;
            expected.init += 1;
            expected.autorelease += if autorelease_skipped(true) { 0 } else { 1 };
            expected.retain += if autorelease_skipped(true) { 0 } else { 1 };
            expected.assert_current();
            res
        });
        expected.release += if autorelease_skipped(true) { 0 } else { 1 };
        expected.assert_current();
    }

    fn create_obj() -> Retained<NSObject> {
        let obj = ManuallyDrop::new(NSObject::new());
        unsafe {
            let obj: *mut NSObject = msg_send![&*obj, autorelease];
            // All code between the `msg_send!` and the `retain_autoreleased`
            // must be able to be optimized away for this to work.
            Retained::retain_autoreleased(obj).unwrap()
        }
    }

    #[test]
    fn test_retain_autoreleased() {
        autoreleasepool(|_| {
            // Run once to allow DYLD to resolve the symbol stubs.
            // Required for making `retain_autoreleased` work on x86_64.
            let _data = create_obj();

            // When compiled in release mode / with optimizations enabled,
            // subsequent usage of `retain_autoreleased` will succeed in
            // retaining the autoreleased value!
            let expected = if autorelease_skipped(false) { 1 } else { 2 };

            let data = create_obj();
            assert_eq!(data.retainCount(), expected);

            let data = create_obj();
            assert_eq!(data.retainCount(), expected);

            // Here we manually clean up the autorelease, so it will always be 1.
            let data = autoreleasepool(|_| create_obj());
            assert_eq!(data.retainCount(), 1);
        });
    }

    #[test]
    fn msg_send_class() {
        let cls = NSObject::class();

        let retained: Retained<AnyClass> = unsafe { msg_send![cls, self] };
        assert_eq!(&*retained, cls);

        let retained: Option<Retained<AnyClass>> = unsafe { msg_send![cls, self] };
        let retained = retained.unwrap();
        assert_eq!(&*retained, cls);
    }

    impl RcTestObject {
        extern_methods!(
            #[unsafe(method(copy))]
            #[unsafe(method_family = new)]
            fn copy_new(&self) -> Retained<Self>;

            #[unsafe(method(copy))]
            #[unsafe(method_family = init)]
            fn copy_init(this: Allocated<Self>) -> Retained<Self>;

            #[unsafe(method(copy))]
            #[unsafe(method_family = copy)]
            fn copy_copy(&self) -> Retained<Self>;

            #[unsafe(method(copy))]
            #[unsafe(method_family = mutableCopy)]
            fn copy_mutable_copy(&self) -> Retained<Self>;

            #[unsafe(method(copy))]
            #[unsafe(method_family = none)]
            fn copy_none(&self) -> Retained<Self>;
        );
    }

    #[test]
    fn test_method_family() {
        // Test a few combinations of (incorrect) method families.
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let copy = obj.copy_new();
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();
        drop(copy);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        let alloc = RcTestObject::alloc();
        let ptr = Allocated::as_ptr(&alloc);
        expected.alloc += 1;
        expected.assert_current();
        let copy = RcTestObject::copy_init(alloc);
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();
        drop(copy);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
        drop(unsafe { Allocated::new(ptr.cast_mut()) });
        expected.release += 1;
        expected.assert_current();

        let copy = obj.copy_copy();
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();
        drop(copy);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        let copy = obj.copy_mutable_copy();
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();
        drop(copy);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        let copy = obj.copy_none();
        expected.copy += 1;
        expected.alloc += 1;
        expected.init += 1;
        expected.retain += 1;
        expected.assert_current();
        // SAFETY: Wrong method family specified, so we have +1 retain count
        // in excess.
        drop(unsafe { Retained::from_raw(Retained::as_ptr(&copy).cast_mut()) });
        expected.release += 1;
        expected.assert_current();
        drop(copy);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

