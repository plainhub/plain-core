
    use alloc::vec::Vec;
    use core::cell::Cell;
    use std::sync::OnceLock;

    use super::*;
    use crate::rc::{Allocated, PartialInit, RcTestObject, Retained, ThreadTestData};
    use crate::runtime::NSObject;
    use crate::{define_class, msg_send, AnyThread, Message};

    /// Initialize superclasses, but not own class.
    unsafe fn init_only_superclasses<T: DefinedClass>(obj: Allocated<T>) -> Retained<T>
    where
        T::Super: ClassType,
    {
        unsafe { Retained::from_raw(msg_send![super(Allocated::into_ptr(obj)), init]) }.unwrap()
    }

    /// Initialize, but fail to finalize (which is done internally by
    /// `msg_send!` when returning `Retained`).
    unsafe fn init_no_finalize<T: DefinedClass>(obj: Allocated<T>) -> Retained<T>
    where
        T::Super: ClassType,
        T::Ivars: Default,
    {
        let obj = obj.set_ivars(Default::default());
        unsafe { Retained::from_raw(msg_send![super(PartialInit::into_ptr(obj)), init]) }.unwrap()
    }

    /// Initialize properly.
    unsafe fn init<T: DefinedClass>(obj: Allocated<T>) -> Retained<T> {
        unsafe { msg_send![obj, init] }
    }

    #[test]
    fn assert_size() {
        assert_eq!(mem::size_of::<DropFlag>(), 1);
    }

    #[test]
    #[cfg(feature = "std")]
    fn test_dealloc_and_dealloc_subclasses() {
        use std::sync::Mutex;

        #[derive(Debug, PartialEq)]
        enum Operation {
            DropIvar,
            DropClass,
        }

        static OPERATIONS: Mutex<Vec<Operation>> = Mutex::new(Vec::new());

        #[derive(Default)]
        struct IvarThatImplsDrop;

        impl Drop for IvarThatImplsDrop {
            fn drop(&mut self) {
                OPERATIONS.lock().unwrap().push(Operation::DropIvar);
            }
        }

        #[track_caller]
        fn check<const N: usize>(expected: [Operation; N]) {
            let mut operations = OPERATIONS.lock().unwrap();
            assert_eq!(&**operations, expected);
            operations.clear();
        }

        // First class

        define_class!(
            #[unsafe(super(NSObject))]
            #[ivars = ()]
            struct ImplsDrop;

            impl ImplsDrop {
                #[unsafe(method_id(init))]
                fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
                    unsafe { msg_send![super(this.set_ivars(())), init] }
                }
            }
        );

        impl Drop for ImplsDrop {
            fn drop(&mut self) {
                OPERATIONS.lock().unwrap().push(Operation::DropClass);
            }
        }

        let _ = ImplsDrop::alloc();
        check([]);

        let _ = unsafe { init_only_superclasses(ImplsDrop::alloc()) };
        check([]);

        let _ = unsafe { init_no_finalize(ImplsDrop::alloc()) };
        check([]);

        let _ = unsafe { init(ImplsDrop::alloc()) };
        check([Operation::DropClass]);

        // Subclass

        define_class!(
            #[unsafe(super(ImplsDrop))]
            #[ivars = IvarThatImplsDrop]
            struct IvarsImplDrop;

            impl IvarsImplDrop {
                #[unsafe(method_id(init))]
                fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
                    unsafe { msg_send![super(this.set_ivars(IvarThatImplsDrop)), init] }
                }
            }
        );

        let _ = IvarsImplDrop::alloc();
        check([]);

        let _ = unsafe { init_only_superclasses(IvarsImplDrop::alloc()) };
        check([Operation::DropClass]);

        let _ = unsafe { init_no_finalize(IvarsImplDrop::alloc()) };
        check([Operation::DropIvar, Operation::DropClass]);

        let _ = unsafe { init(IvarsImplDrop::alloc()) };
        check([Operation::DropIvar, Operation::DropClass]);

        // Further subclass

        define_class!(
            #[unsafe(super(IvarsImplDrop))]
            #[ivars = IvarThatImplsDrop]
            struct BothIvarsAndTypeImplsDrop;

            impl BothIvarsAndTypeImplsDrop {
                #[unsafe(method_id(init))]
                fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
                    unsafe { msg_send![super(this.set_ivars(IvarThatImplsDrop)), init] }
                }
            }
        );

        impl Drop for BothIvarsAndTypeImplsDrop {
            fn drop(&mut self) {
                OPERATIONS.lock().unwrap().push(Operation::DropClass);
            }
        }

        let _ = BothIvarsAndTypeImplsDrop::alloc();
        check([]);

        let _ = unsafe { init_only_superclasses(BothIvarsAndTypeImplsDrop::alloc()) };
        check([Operation::DropIvar, Operation::DropClass]);

        let _ = unsafe { init_no_finalize(BothIvarsAndTypeImplsDrop::alloc()) };
        check([
            Operation::DropIvar,
            Operation::DropIvar,
            Operation::DropClass,
        ]);

        let _ = unsafe { init(BothIvarsAndTypeImplsDrop::alloc()) };
        check([
            Operation::DropClass,
            Operation::DropIvar,
            Operation::DropIvar,
            Operation::DropClass,
        ]);
    }

    #[test]
    fn test_no_generated_dealloc_if_not_needed() {
        #[allow(unused)]
        struct Ivar {
            field1: u8,
            field2: bool,
        }

        define_class!(
            #[unsafe(super(NSObject))]
            #[ivars = Ivar]
            struct IvarsNoDrop;
        );

        assert!(!mem::needs_drop::<IvarsNoDrop>());
        assert!(!mem::needs_drop::<Ivar>());
        assert_eq!(
            IvarsNoDrop::class().instance_method(sel!(dealloc)),
            NSObject::class().instance_method(sel!(dealloc)),
        );
    }

    #[test]
    fn zst_ivar() {
        #[derive(Default, Debug, Clone, Copy)]
        struct Ivar;

        define_class!(
            #[unsafe(super(NSObject))]
            #[ivars = Cell<Ivar>]
            struct IvarZst;

            impl IvarZst {
                #[unsafe(method_id(init))]
                fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
                    unsafe { msg_send![super(this.set_ivars(Cell::new(Ivar))), init] }
                }
            }
        );

        assert_eq!(
            IvarZst::class().instance_size(),
            NSObject::class().instance_size(),
        );
        let ivar_name = if cfg!(any()) {
            "IvarZst_ivars"
        } else {
            "ivars"
        };
        let ivar_name = CString::new(ivar_name).unwrap();
        assert!(IvarZst::class().instance_variable(&ivar_name).is_none());

        let obj = unsafe { init(IvarZst::alloc()) };
        #[cfg(feature = "std")]
        std::println!("{:?}", obj.ivars().get());
        obj.ivars().set(Ivar);
    }

    #[test]
    #[should_panic = "unsupported alignment 16 for `HasIvarWithHighAlignment::Ivars`"]
    fn test_generate_ivar_high_alignment() {
        #[repr(align(16))]
        struct HighAlignment;

        define_class!(
            #[unsafe(super(NSObject))]
            #[name = "HasIvarWithHighAlignment"]
            #[ivars = HighAlignment]
            struct HasIvarWithHighAlignment;
        );

        // Have to allocate up to the desired alignment, but no need to go
        // further, since the object is zero-sized.
        assert_eq!(HasIvarWithHighAlignment::class().instance_size(), 16);

        let ivar_name = if cfg!(any()) {
            "IvarZst_ivars"
        } else {
            "ivars"
        };
        let ivar_name = CString::new(ivar_name).unwrap();
        let ivar = HasIvarWithHighAlignment::class()
            .instance_variable(&ivar_name)
            .unwrap();
        assert_eq!(ivar.offset(), 16);
    }

    #[test]
    fn test_ivar_access() {
        define_class!(
            #[unsafe(super(NSObject))]
            #[ivars = Cell<Option<Retained<RcTestObject>>>]
            struct RcIvar;

            impl RcIvar {
                #[unsafe(method_id(init))]
                fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
                    let this = this.set_ivars(Cell::new(Some(RcTestObject::new())));
                    unsafe { msg_send![super(this), init] }
                }
            }
        );

        let mut expected = ThreadTestData::current();

        let _ = RcIvar::alloc();
        expected.assert_current();

        let _ = unsafe { init_only_superclasses(RcIvar::alloc()) };
        expected.assert_current();

        // Ivar access is valid even if the class is not finalized.
        let obj = unsafe { init_no_finalize(RcIvar::alloc()) };
        expected.assert_current();

        obj.ivars().set(Some(RcTestObject::new()));
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        drop(obj);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        let obj = unsafe { init(RcIvar::alloc()) };
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        // SAFETY: Cloned immediately after, so not accessed while borrowed
        let ivar = unsafe { &*obj.ivars().as_ptr() }.clone();
        obj.ivars().set(ivar);
        expected.retain += 1;
        expected.release += 1;
        expected.assert_current();

        drop(obj);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        #[derive(Default)]
        struct RcIvarSubclassIvars {
            int: Cell<i32>,
            obj: Cell<Retained<RcTestObject>>,
        }

        define_class!(
            #[unsafe(super(RcIvar))]
            #[ivars = RcIvarSubclassIvars]
            struct RcIvarSubclass;

            impl RcIvarSubclass {
                #[unsafe(method_id(init))]
                fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
                    let this = this.set_ivars(RcIvarSubclassIvars {
                        int: Cell::new(42),
                        obj: Cell::new(RcTestObject::new()),
                    });
                    unsafe { msg_send![super(this), init] }
                }
            }
        );

        let obj = unsafe { init(RcIvarSubclass::alloc()) };
        expected.alloc += 2;
        expected.init += 2;
        expected.assert_current();
        assert_eq!(obj.ivars().int.get(), 42);

        obj.ivars().int.set(obj.ivars().int.get() + 1);
        assert_eq!(obj.ivars().int.get(), 43);

        // SAFETY: Cloned immediately after, so not accessed while borrowed
        let ivar = unsafe { &*(**obj).ivars().as_ptr() }.clone().unwrap();
        obj.ivars().obj.set(ivar);
        expected.retain += 1;
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        // Change super ivars
        (**obj).ivars().set(None);
        expected.release += 1;
        expected.assert_current();

        drop(obj);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();

        let obj = unsafe { init_only_superclasses(RcIvarSubclass::alloc()) };
        expected.alloc += 1;
        expected.init += 1;
        expected.assert_current();

        // Accessing superclass ivars is valid
        // SAFETY: Cell not accessed while ivar is borrowed
        #[cfg(feature = "std")]
        std::println!("{:?}", unsafe { &*(**obj).ivars().as_ptr() });

        drop(obj);
        expected.release += 1;
        expected.drop += 1;
        expected.assert_current();
    }

    #[test]
    #[cfg_attr(not(debug_assertions), ignore = "only panics with debug assertions")]
    #[should_panic = "tried to access uninitialized instance variable"]
    fn access_invalid() {
        define_class!(
            #[unsafe(super(NSObject))]
            // Type has to have a drop flag to detect invalid access
            #[ivars = Retained<NSObject>]
            struct InvalidAccess;
        );

        let obj = unsafe { init_only_superclasses(InvalidAccess::alloc()) };
        #[cfg(feature = "std")]
        std::println!("{:?}", obj.ivars());
    }

    #[test]
    #[should_panic = "panic in drop"]
    #[ignore = "panicking in Drop requires that we actually implement `dealloc` as `C-unwind`"]
    fn test_panic_in_drop() {
        define_class!(
            #[unsafe(super(NSObject))]
            struct DropPanics;
        );

        impl Drop for DropPanics {
            fn drop(&mut self) {
                panic!("panic in drop");
            }
        }

        let obj = DropPanics::alloc().set_ivars(());
        let obj: Retained<DropPanics> = unsafe { msg_send![super(obj), init] };
        drop(obj);
    }

    #[test]
    #[should_panic = "panic in ivar drop"]
    #[ignore = "panicking in Drop requires that we actually implement `dealloc` as `C-unwind`"]
    fn test_panic_in_ivar_drop() {
        struct DropPanics;

        impl Drop for DropPanics {
            fn drop(&mut self) {
                panic!("panic in ivar drop");
            }
        }

        define_class!(
            #[unsafe(super(NSObject))]
            #[ivars = DropPanics]
            struct IvarDropPanics;
        );

        let obj = IvarDropPanics::alloc().set_ivars(DropPanics);
        let obj: Retained<IvarDropPanics> = unsafe { msg_send![super(obj), init] };
        drop(obj);
    }

    // We cannot really guard against this, so dealloc must be unsafe!
    //
    // At least not by checking `retainCount`, since that gets set to `0` when
    // dropping. I guess we _could_ override `retain` and check `retainCount`
    // before we do anything, but that seems brittle, and it would hurt
    // performance.
    #[test]
    fn test_retain_leak_in_drop() {
        define_class!(
            // SAFETY: Intentionally broken!
            #[unsafe(super(NSObject))]
            #[derive(Debug)]
            struct DropRetainsAndLeaksSelf;
        );

        unsafe impl Send for DropRetainsAndLeaksSelf {}
        unsafe impl Sync for DropRetainsAndLeaksSelf {}

        static OBJ: OnceLock<Retained<DropRetainsAndLeaksSelf>> = OnceLock::new();

        impl Drop for DropRetainsAndLeaksSelf {
            fn drop(&mut self) {
                fn inner(this: &DropRetainsAndLeaksSelf) {
                    // Smuggle a reference out of this context.
                    OBJ.set(this.retain()).unwrap();
                }

                inner(self)
            }
        }

        let obj = DropRetainsAndLeaksSelf::alloc().set_ivars(());
        let obj: Retained<DropRetainsAndLeaksSelf> = unsafe { msg_send![super(obj), init] };
        drop(obj);

        // Suddenly, the object is alive again!
        let _ = OBJ.get().unwrap();
    }

