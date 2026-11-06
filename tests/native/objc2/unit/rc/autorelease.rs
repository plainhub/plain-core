
    use core::mem;
    use core::panic::{AssertUnwindSafe, RefUnwindSafe, UnwindSafe};
    use std::panic::catch_unwind;

    use static_assertions::{assert_impl_all, assert_not_impl_any};

    use super::{autoreleasepool, AutoreleasePool, AutoreleaseSafe};
    use crate::rc::{RcTestObject, Retained, ThreadTestData};
    use crate::runtime::AnyObject;

    #[test]
    fn auto_traits() {
        assert_impl_all!(AutoreleasePool<'static>: Unpin, UnwindSafe, RefUnwindSafe);
        assert_not_impl_any!(AutoreleasePool<'static>: Send, Sync);

        assert_impl_all!(usize: AutoreleaseSafe);
        assert_impl_all!(*mut AnyObject: AutoreleaseSafe);
        assert_impl_all!(&mut AnyObject: AutoreleaseSafe);
        #[cfg(any())]
        assert_not_impl_any!(AutoreleasePool<'static>: AutoreleaseSafe);
    }

    #[allow(unused)]
    fn assert_covariant1<'a>(pool: AutoreleasePool<'static>) -> AutoreleasePool<'a> {
        pool
    }

    #[allow(unused)]
    fn assert_covariant2<'long: 'short, 'short>(
        pool: AutoreleasePool<'long>,
    ) -> AutoreleasePool<'short> {
        pool
    }

    #[allow(unused)]
    fn assert_object_safe(_: &dyn AutoreleaseSafe) {}

    #[cfg_attr(
        not(any()),
        ignore = "only stably ZST when `unstable-autoreleasesafe` is enabled"
    )]
    #[test]
    fn assert_zst() {
        assert_eq!(mem::size_of::<AutoreleasePool<'static>>(), 0);
    }

    #[test]
    #[cfg_attr(panic = "abort", ignore = "requires `catch_unwind`")]
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86", not(panic = "abort")),
        ignore = "unwinding through an auto release pool on macOS 32 bit won't pop the pool"
    )]
    fn test_unwind_still_autoreleases() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        catch_unwind({
            let obj = AssertUnwindSafe(obj);
            let expected = AssertUnwindSafe(&mut expected);
            || {
                let obj = obj;
                let mut expected = expected;

                autoreleasepool(|pool| {
                    let _autoreleased = unsafe { Retained::autorelease(obj.0, pool) };
                    expected.autorelease += 1;
                    expected.assert_current();
                    panic!("unwind");
                });
            }
        })
        .unwrap_err();

        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

