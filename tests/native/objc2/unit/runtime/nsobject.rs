
    use super::*;
    use alloc::format;

    use crate::extern_class;
    use crate::rc::RcTestObject;

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "NSObject"]
        #[derive(Debug, PartialEq, Eq, Hash)]
        struct FakeSubclass;
    );

    impl FakeSubclass {
        fn new() -> Retained<Self> {
            Retained::downcast(NSObject::new()).unwrap()
        }
    }

    #[test]
    fn test_deref() {
        let obj: Retained<FakeSubclass> = FakeSubclass::new();
        let _: &FakeSubclass = &obj;
        let _: &NSObject = &obj;
        let _: &AnyObject = &obj;
    }

    #[test]
    fn test_as_ref_borrow() {
        use core::borrow::Borrow;

        fn impls_as_ref_borrow<T: AsRef<U> + Borrow<U> + ?Sized, U: ?Sized>(_: &T) {}

        let obj = FakeSubclass::new();
        impls_as_ref_borrow::<Retained<FakeSubclass>, FakeSubclass>(&obj);
        impls_as_ref_borrow::<FakeSubclass, FakeSubclass>(&obj);
        impls_as_ref_borrow::<NSObject, NSObject>(&obj);
        impls_as_ref_borrow::<NSObject, AnyObject>(&obj);

        let obj = NSObject::new();
        impls_as_ref_borrow::<Retained<NSObject>, NSObject>(&obj);

        fn impls_as_ref<T: AsRef<U> + ?Sized, U: ?Sized>(_: &T) {}

        let obj = FakeSubclass::new();
        impls_as_ref::<Retained<FakeSubclass>, FakeSubclass>(&obj);
        impls_as_ref::<Retained<FakeSubclass>, NSObject>(&obj);
        impls_as_ref::<Retained<FakeSubclass>, AnyObject>(&obj);
    }

    #[test]
    fn test_equality() {
        let obj1 = NSObject::new();
        assert_eq!(obj1, obj1);

        let obj2 = NSObject::new();
        assert_ne!(obj1, obj2);
    }

    #[test]
    fn test_hash() {
        use core::hash::Hasher;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;

        let obj1 = NSObject::new();

        let mut hashstate1 = DefaultHasher::new();
        let mut hashstate2 = DefaultHasher::new();

        obj1.hash(&mut hashstate1);
        obj1.hash(&mut hashstate2);

        assert_eq!(hashstate1.finish(), hashstate2.finish());

        let obj2 = NSObject::new();
        let mut hashstate2 = DefaultHasher::new();
        obj2.hash(&mut hashstate2);
        assert_ne!(hashstate1.finish(), hashstate2.finish());
    }

    #[test]
    fn test_debug() {
        let obj = NSObject::new();
        let expected = format!("<NSObject: {:p}>", &*obj);
        assert_eq!(format!("{obj:?}"), expected);
    }

    #[test]
    fn test_is_kind_of() {
        let obj = NSObject::new();
        assert!(obj.isKindOfClass(NSObject::class()));
        assert!(!obj.isKindOfClass(RcTestObject::class()));

        let obj = RcTestObject::new();
        assert!(obj.isKindOfClass(NSObject::class()));
        assert!(obj.isKindOfClass(RcTestObject::class()));
    }

    #[test]
    fn test_retain_same() {
        let obj1 = NSObject::new();
        let ptr1 = Retained::as_ptr(&obj1);

        let obj2 = obj1.clone();
        let ptr2 = Retained::as_ptr(&obj2);

        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn conforms_to_nsobjectprotocol() {
        let protocol = <dyn NSObjectProtocol>::protocol().unwrap();
        assert!(NSObject::class().conforms_to(protocol));
    }

    // Ensure that importing `NSObjectProtocol::hash` does not cause conflicts
    // when using `Hash::hash` on normal types.
    mod hash_does_not_overlap_with_normal_hash_method {
        #[allow(unused_imports)]
        use crate::runtime::NSObjectProtocol;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;

        #[test]
        fn inner() {
            let integer = 5;
            let mut hasher = DefaultHasher::new();
            integer.hash(&mut hasher);
        }
    }

