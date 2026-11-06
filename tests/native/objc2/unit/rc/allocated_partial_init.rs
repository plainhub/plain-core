
    use core::panic::{RefUnwindSafe, UnwindSafe};

    use static_assertions::assert_not_impl_any;

    use super::*;
    use crate::rc::RcTestObject;
    use crate::runtime::NSObject;

    #[test]
    fn auto_traits() {
        assert_not_impl_any!(Allocated<()>: Send, Sync, UnwindSafe, RefUnwindSafe, Unpin);
        assert_not_impl_any!(PartialInit<()>: Send, Sync, UnwindSafe, RefUnwindSafe, Unpin);
    }

    #[repr(C)]
    struct MyObject<'a> {
        inner: NSObject,
        p: PhantomData<&'a str>,
    }

    /// Test that `Allocated<T>` is covariant over `T`.
    #[allow(unused)]
    fn assert_allocated_variance<'b>(obj: Allocated<MyObject<'static>>) -> Allocated<MyObject<'b>> {
        obj
    }

    /// Test that `PartialInit<T>` is covariant over `T`.
    #[allow(unused)]
    fn assert_partialinit_variance<'b>(
        obj: PartialInit<MyObject<'static>>,
    ) -> PartialInit<MyObject<'b>> {
        obj
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic = "tried to initialize instance variables on a NULL allocated object"
    )]
    fn test_set_ivars_null() {
        // SAFETY: The pointer is NULL
        let obj: Allocated<RcTestObject> = unsafe { Allocated::new(ptr::null_mut()) };
        let _ = obj.set_ivars(());
    }

    #[test]
    #[cfg(any())]
    fn arbitrary_self_types() {
        use crate::rc::Retained;
        use crate::{extern_methods, AnyThread};

        impl RcTestObject {
            extern_methods!(
                #[unsafe(method(init))]
                fn init_with_self(self: Allocated<Self>) -> Retained<Self>;
            );
        }

        let _ = RcTestObject::alloc().init_with_self();
    }

