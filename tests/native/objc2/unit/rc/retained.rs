
    use core::mem::size_of;

    use static_assertions::{assert_impl_all, assert_not_impl_any};

    use super::*;
    use crate::rc::{autoreleasepool, RcTestObject, ThreadTestData};
    use crate::runtime::{AnyObject, NSObject, NSObjectProtocol};
    use crate::{define_class, msg_send};

    #[test]
    fn auto_traits() {
        macro_rules! helper {
            ($name:ident) => {
                define_class!(
                    #[unsafe(super(NSObject))]
                    #[name = concat!(stringify!($name), "Test")]
                    // Make the type not thread safe by default.
                    #[ivars = *const ()]
                    struct $name;
                );
            };
        }

        helper!(Object);
        helper!(SendObject);
        unsafe impl Send for SendObject {}
        helper!(SyncObject);
        unsafe impl Sync for SyncObject {}
        helper!(SendSyncObject);
        unsafe impl Send for SendSyncObject {}
        unsafe impl Sync for SendSyncObject {}

        assert_impl_all!(Retained<AnyObject>: Unpin);
        assert_not_impl_any!(Retained<AnyObject>: Send, Sync, UnwindSafe, RefUnwindSafe);

        assert_not_impl_any!(Retained<Object>: Send, Sync);
        assert_not_impl_any!(Retained<SendObject>: Send, Sync);
        assert_not_impl_any!(Retained<SyncObject>: Send, Sync);
        assert_impl_all!(Retained<SendSyncObject>: Send, Sync);
    }

    #[test]
    fn test_drop() {
        let mut expected = ThreadTestData::current();

        let obj = RcTestObject::new();
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        drop(obj);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    fn test_autorelease() {
        let obj = RcTestObject::new();
        let cloned = obj.clone();
        let mut expected = ThreadTestData::current();

        autoreleasepool(|pool| {
            let _ref = unsafe { Retained::autorelease(obj, pool) };
            expected.autorelease += 1;
            expected.assert_current();
            assert_eq!(cloned.retainCount(), 2);
        });
        expected.release += 1;
        expected.assert_current();
        assert_eq!(cloned.retainCount(), 1);

        autoreleasepool(|pool| {
            let _ref = unsafe { Retained::autorelease(cloned, pool) };
            expected.autorelease += 1;
            expected.assert_current();
        });
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    fn test_clone() {
        let obj = RcTestObject::new();
        assert_eq!(obj.retainCount(), 1);
        let mut expected = ThreadTestData::current();

        expected.assert_current();
        assert_eq!(obj.retainCount(), 1);

        let cloned = obj.clone();
        expected.retain += 1;
        expected.assert_current();
        assert_eq!(cloned.retainCount(), 2);
        assert_eq!(obj.retainCount(), 2);

        let obj = obj.into_super().into_super();
        let cloned_and_type_erased = obj.clone();
        expected.retain += 1;
        expected.assert_current();
        let retain_count: usize = unsafe { msg_send![&cloned_and_type_erased, retainCount] };
        assert_eq!(retain_count, 3);
        let retain_count: usize = unsafe { msg_send![&obj, retainCount] };
        assert_eq!(retain_count, 3);

        drop(obj);
        expected.release += 1;
        expected.assert_current();
        assert_eq!(cloned.retainCount(), 2);

        drop(cloned_and_type_erased);
        expected.release += 1;
        expected.assert_current();
        assert_eq!(cloned.retainCount(), 1);

        drop(cloned);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    fn test_retain_autoreleased_works_as_retain() {
        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let ptr = Retained::as_ptr(&obj) as *mut RcTestObject;
        let _obj2 = unsafe { Retained::retain_autoreleased(ptr) }.unwrap();
        expected.retain += 1;
        expected.assert_current();
    }

    #[test]
    fn test_cast() {
        let obj: Retained<RcTestObject> = RcTestObject::new();
        let expected = ThreadTestData::current();

        let obj: Retained<AnyObject> = obj.into();
        expected.assert_current();

        let _obj: Retained<RcTestObject> = Retained::downcast(obj).unwrap();
        expected.assert_current();
    }

    #[repr(C)]
    struct MyObject<'a> {
        inner: NSObject,
        p: PhantomData<&'a str>,
    }

    /// Test that `Retained<T>` is covariant over `T`.
    #[allow(unused)]
    fn assert_retained_variance<'b>(obj: Retained<MyObject<'static>>) -> Retained<MyObject<'b>> {
        obj
    }

    #[test]
    fn test_size_of() {
        let ptr_size = size_of::<&NSObject>();

        assert_eq!(size_of::<Retained<NSObject>>(), ptr_size);
        assert_eq!(size_of::<Option<Retained<NSObject>>>(), ptr_size);
    }

    #[test]
    fn test_into() {
        let obj = NSObject::new();
        let obj: Retained<NSObject> = Into::into(obj);
        let _: Retained<AnyObject> = Into::into(obj);

        let obj_ref = &*NSObject::new();
        let _: Retained<NSObject> = Into::into(obj_ref);
        let _: Retained<AnyObject> = Into::into(obj_ref);

        let obj_retained_ref = &NSObject::new();
        let _: Retained<NSObject> = Into::into(obj_retained_ref);
        let _: Retained<AnyObject> = Into::into(obj_retained_ref);

        let protocol_obj = ProtocolObject::<dyn NSObjectProtocol>::from_retained(NSObject::new());
        let _: Retained<AnyObject> = Into::into(protocol_obj);
    }

    #[test]
    #[cfg(any())]
    fn test_coercion() {
        use crate::extern_protocol;

        extern_protocol!(
            unsafe trait ExampleProtocol: NSObjectProtocol {}
        );

        unsafe impl ExampleProtocol for RcTestObject {}

        let obj = RcTestObject::new();
        let mut expected = ThreadTestData::current();

        let obj: Retained<dyn ExampleProtocol> = obj;
        expected.assert_current();

        let obj: Retained<dyn NSObjectProtocol> = obj;
        expected.assert_current();

        // TODO: Allow calling methods on trait objects like this.
        // let _ = obj.hash();

        drop(obj);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    // Regression test for https://github.com/madsmtm/objc2/issues/861.
    #[test]
    fn retain_autoreleased_not_tail_called() {
        // If broken, this will get compiled as:
        //
        // ```
        // _retain_autoreleased:
        //     mov x29, x29
        //     b _objc_retainAutoreleasedReturnValue
        //
        // _trigger:
        //     bl _objc_autorelease
        //     mov x0, x1
        //     bl _retain_autoreleased
        //     b _objc_release
        // ```
        //
        // And that would cause `objc_retainAutoreleasedReturnValue` to see
        // its return address at `bl _retain_autoreleased`, look two
        // instructions up, and find that it matches the return address from
        // `bl _objc_autorelease`.
        //
        // (It doesn't try to read the intermediate `mov` instruction, so it
        // doesn't know that the objects don't match).
        #[inline(never)]
        fn retain_autoreleased(obj: &NSObject) -> Retained<NSObject> {
            let obj: *const NSObject = obj;
            let obj = unsafe { Retained::retain_autoreleased(obj.cast_mut()) };
            unsafe { obj.unwrap_unchecked() }
        }

        #[inline(never)]
        fn trigger(a: Retained<NSObject>, b: &NSObject) {
            let _ = Retained::autorelease_ptr(a);
            let b = retain_autoreleased(b);
            // Helps ensure ^ is not tail-called.
            drop(b);
        }

        let a = NSObject::new();
        let b = NSObject::new();

        let a_clone = a.clone();
        // Forget `b` when unwinding make this easier to debug.
        let b_clone = ManuallyDrop::new(b.clone());

        // We've cloned twice.
        assert_eq!(a.retainCount(), 2);
        assert_eq!(b.retainCount(), 2);

        autoreleasepool(|_| {
            trigger(a_clone, &b);

            // `a` should be autoreleased, and so not yet dropped.
            assert_eq!(a.retainCount(), 2);
            // `b` should be retained and dropped, so no change.
            assert_eq!(b.retainCount(), 2);
        });

        // And now `a` should've been dropped and `b` still unchanged.
        assert_eq!(a.retainCount(), 1);
        assert_eq!(b.retainCount(), 2);

        let _ = ManuallyDrop::into_inner(b_clone);
    }

    /// Regression test for <https://github.com/madsmtm/objc2/issues/861>.
    ///
    /// Specifically the bug that `objc_autorelease` only checks that the next
    /// instruction is `mov x29, x29`, not that any further code is.
    #[test]
    fn retain_autoreleased_called_with_different_argument() {
        // If broken, this will get compiled as:
        //
        // ```
        // bl _objc_autorelease
        // mov x29, x29
        // mov x0, x1
        // bl _objc_retainAutoreleasedReturnValue
        // nop
        // b _objc_release
        // ```
        //
        // Specifically, with the `mov x29, x29` right after `objc_autorelease`,
        // instead of before `objc_retainAutoreleasedReturnValue`.
        //
        // This is a problem, because it `objc_autorelease` checks whether the
        // next instruction is that, and if it is, it performs the
        // optimization (which is incorrect, `a` and `b` are different!)
        #[inline(never)]
        fn trigger(a: Retained<NSObject>, b: &NSObject) {
            let _ = Retained::autorelease_ptr(a);
            let b = unsafe { Retained::retain_autoreleased(b as *const NSObject as *mut NSObject) };
            let b = unsafe { b.unwrap_unchecked() };
            drop(b);
        }

        let a = NSObject::new();
        let b = NSObject::new();

        let a_clone = a.clone();
        // Forget `b` when unwinding make this easier to debug.
        let b_clone = ManuallyDrop::new(b.clone());

        // We've cloned twice.
        assert_eq!(a.retainCount(), 2);
        assert_eq!(b.retainCount(), 2);

        autoreleasepool(|_| {
            trigger(a_clone, &b);

            // `a` should be autoreleased, and so not yet dropped.
            assert_eq!(a.retainCount(), 2);
            // `b` should be retained and dropped, so no change.
            assert_eq!(b.retainCount(), 2);
        });

        // And now `a` should've been dropped and `b` still unchanged.
        assert_eq!(a.retainCount(), 1);
        assert_eq!(b.retainCount(), 2);

        let _ = ManuallyDrop::into_inner(b_clone);
    }

