
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::*;
    use crate::rc::{autoreleasepool, Allocated, RcTestObject, ThreadTestData};
    use crate::{msg_send, ClassType};

    #[test]
    fn test_bool_error() {
        let mut expected = ThreadTestData::current();

        fn bool_error(should_error: bool, error: Option<&mut Option<Retained<RcTestObject>>>) {
            let cls = RcTestObject::class();
            let did_succeed: bool =
                unsafe { msg_send![cls, boolAndShouldError: should_error, error: error] };
            assert_ne!(should_error, did_succeed);
        }

        bool_error(false, None);
        bool_error(true, None);
        expected.assert_current();

        fn helper(
            expected: &mut ThreadTestData,
            should_error: bool,
            mut error: Option<Retained<RcTestObject>>,
        ) {
            autoreleasepool(|_| {
                bool_error(should_error, Some(&mut error));
                if should_error {
                    expected.alloc += 1;
                    expected.init += 1;
                    expected.autorelease += 1;
                }
                expected.assert_current();
            });

            if should_error {
                expected.release += 1;
            }
            expected.assert_current();

            if error.is_some() {
                expected.release += 1;
                expected.drop += 1;
            }
            drop(error);
            expected.assert_current();
        }

        helper(&mut expected, false, None);

        expected.retain += 1;
        helper(&mut expected, true, None);

        expected.alloc += 1;
        expected.init += 1;
        expected.retain += 1;
        expected.release += 1;
        helper(&mut expected, false, Some(RcTestObject::new()));

        expected.alloc += 1;
        expected.init += 1;
        expected.retain += 1;
        expected.release += 1;
        expected.drop += 1;
        helper(&mut expected, true, Some(RcTestObject::new()));
    }

    #[test]
    #[cfg_attr(
        any(not(debug_assertions), all(not(target_pointer_width = "64"), any())),
        ignore = "invokes UB which is only caught with debug_assertions"
    )]
    #[should_panic = "found that NULL was written to `&mut Retained<_>`, which is UB! You should handle this with `&mut Option<Retained<_>>` instead"]
    fn test_debug_check_ub() {
        let cls = RcTestObject::class();
        let mut param: Retained<_> = RcTestObject::new();
        let _: () = unsafe { msg_send![cls, outParamNull: &mut param] };
    }

    // TODO: Fix this in release mode with Apple's runtime
    const AUTORELEASE_SKIPPED: bool = cfg!(any());

    #[test]
    fn test_retained_interaction() {
        let mut expected = ThreadTestData::current();
        let cls = RcTestObject::class();

        let mut err: Retained<RcTestObject> = RcTestObject::new();
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        autoreleasepool(|_| {
            let obj: Option<Retained<RcTestObject>> =
                unsafe { msg_send![cls, idAndShouldError: false, error: &mut err] };
            expected.alloc += 1;
            expected.init += 1;
            if !AUTORELEASE_SKIPPED {
                expected.autorelease += 1;
                expected.retain += 1;
            }

            expected.retain += 1;
            expected.release += 1;
            expected.assert_current();

            drop(obj);
            expected.release += 1;
            if AUTORELEASE_SKIPPED {
                expected.drop += 1;
            }
            expected.assert_current();
        });
        if !AUTORELEASE_SKIPPED {
            expected.release += 1;
            expected.drop += 1;
        }
        expected.assert_current();

        drop(err);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    fn test_error_alloc() {
        let mut expected = ThreadTestData::current();

        // Succeeds
        let mut error: Option<Retained<RcTestObject>> = None;
        let res: Allocated<RcTestObject> = unsafe {
            msg_send![RcTestObject::class(), allocAndShouldError: false, error: &mut error]
        };
        expected.alloc += 1;
        expected.assert_current();
        assert!(!Allocated::as_ptr(&res).is_null());
        assert!(error.is_none());

        drop(res);
        expected.release += 1;
        // Drop flag ensures uninitialized do not drop
        // expected.drop += 1;
        expected.assert_current();

        // Errors
        let res: Retained<RcTestObject> = autoreleasepool(|_pool| {
            let mut error = None;
            let res: Allocated<RcTestObject> = unsafe {
                msg_send![RcTestObject::class(), allocAndShouldError: true, error: &mut error]
            };
            expected.alloc += 1;
            expected.init += 1;
            expected.autorelease += 1;
            expected.retain += 1;
            expected.assert_current();
            assert!(Allocated::as_ptr(&res).is_null());
            error.unwrap()
        });
        expected.release += 1;
        expected.assert_current();

        drop(res);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    fn will_panic(param: Option<&mut Option<Retained<RcTestObject>>>, panic_after: bool) {
        unsafe { msg_send![RcTestObject::class(), willPanicWith: param, panicsAfter: panic_after] }
    }

    #[test]
    #[cfg_attr(
        any(),
        ignore = "panics intentionally, which catch-all interferes with"
    )]
    fn basic_method_panics() {
        let expected = ThreadTestData::current();

        let res = catch_unwind(|| {
            will_panic(None, false);
        });
        assert!(res.is_err());
        expected.assert_current();

        let res = catch_unwind(|| {
            will_panic(None, true);
        });
        assert!(res.is_err());
        expected.assert_current();
    }

    #[test]
    #[cfg_attr(any(any(), panic = "abort"), ignore = "panics intentionally")]
    fn method_panics() {
        let cases = [
            (false, None),
            (true, None),
            // Pre-existing parameter passed in.
            (false, Some(RcTestObject::new())),
            (true, Some(RcTestObject::new())),
        ];

        let mut expected = ThreadTestData::current();

        for (panic_after, mut param) in cases {
            let initially_set = param.is_some();

            autoreleasepool(|_| {
                let unwindsafe = AssertUnwindSafe(&mut param);
                let res = catch_unwind(|| {
                    let param = unwindsafe;
                    will_panic(Some(param.0), panic_after);
                });
                assert!(res.is_err());

                if panic_after {
                    expected.alloc += 1;
                    expected.init += 1;
                    expected.autorelease += 1;
                }
                if panic_after || initially_set {
                    expected.retain += 1;
                }
                if initially_set {
                    expected.release += 1;
                    if panic_after {
                        expected.drop += 1;
                    }
                }
                expected.assert_current();
            });

            if panic_after {
                expected.release += 1;
            }
            expected.assert_current();

            drop(param);
            if panic_after || initially_set {
                expected.release += 1;
                expected.drop += 1;
            }
            expected.assert_current();
        }
    }

