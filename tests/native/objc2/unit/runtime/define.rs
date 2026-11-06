
    use core::hash::Hasher;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;

    use memoffset::offset_of;

    use super::*;
    use crate::encode::RefEncode;
    use crate::rc::Retained;
    use crate::runtime::{NSObject, NSObjectProtocol};
    use crate::{define_class, extern_methods, msg_send, test_utils, ClassType, ProtocolType};

    // TODO: Remove once c"" strings are in MSRV
    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn test_alignment() {
        assert_eq!(<()>::LOG2_ALIGNMENT, 0);

        assert_eq!(u8::LOG2_ALIGNMENT, 0);
        assert_eq!(u16::LOG2_ALIGNMENT, 1);
        assert_eq!(u32::LOG2_ALIGNMENT, 2);

        assert_eq!(
            u64::LOG2_ALIGNMENT,
            if cfg!(target_pointer_width = "64") {
                3
            } else {
                2
            }
        );

        #[repr(align(16))]
        struct Align16;
        assert_eq!(Align16::LOG2_ALIGNMENT, 4);

        #[repr(align(32))]
        struct Align32;
        assert_eq!(Align32::LOG2_ALIGNMENT, 5);

        #[repr(align(64))]
        struct Align64;
        assert_eq!(Align64::LOG2_ALIGNMENT, 6);

        #[repr(align(128))]
        struct Align128;
        assert_eq!(Align128::LOG2_ALIGNMENT, 7);

        #[repr(align(256))]
        struct Align256;
        assert_eq!(Align256::LOG2_ALIGNMENT, 8);

        #[repr(align(536870912))]
        struct Align536870912;
        assert_eq!(Align536870912::LOG2_ALIGNMENT, 29);
    }

    #[test]
    fn test_classbuilder_duplicate() {
        let cls = test_utils::custom_class();
        let builder = ClassBuilder::new(&c("TestClassBuilderDuplicate"), cls).unwrap();
        let _ = builder.register();

        assert!(ClassBuilder::new(&c("TestClassBuilderDuplicate"), cls).is_none());
    }

    #[test]
    #[should_panic = "failed to add ivar \"xyz\""]
    fn duplicate_ivar() {
        let cls = test_utils::custom_class();
        let mut builder = ClassBuilder::new(&c("TestClassBuilderDuplicateIvar"), cls).unwrap();

        builder.add_ivar::<i32>(&c("xyz"));
        // Should panic:
        builder.add_ivar::<i32>(&c("xyz"));
    }

    #[test]
    #[should_panic = "failed to add method xyz"]
    fn duplicate_method() {
        let cls = test_utils::custom_class();
        let mut builder = ClassBuilder::new(&c("TestClassBuilderDuplicateMethod"), cls).unwrap();

        extern "C" fn xyz(_this: &NSObject, _cmd: Sel) {}

        unsafe {
            builder.add_method(sel!(xyz), xyz as extern "C" fn(_, _));
            // Should panic:
            builder.add_method(sel!(xyz), xyz as extern "C" fn(_, _));
        }
    }

    #[test]
    #[should_panic = "selector xyz: accepts 1 arguments, but function accepts 0"]
    fn wrong_arguments() {
        let cls = test_utils::custom_class();
        let mut builder = ClassBuilder::new(&c("TestClassBuilderWrongArguments"), cls).unwrap();

        extern "C" fn xyz(_this: &NSObject, _cmd: Sel) {}

        unsafe {
            // Should panic:
            builder.add_method(sel!(xyz:), xyz as extern "C" fn(_, _));
        }
    }

    #[test]
    #[cfg_attr(
        all(debug_assertions, not(feature = "disable-encoding-assertions")),
        should_panic = "defined invalid method -[TestClassBuilderInvalidMethod foo]: expected return to have type code 'I', but found 's'"
    )]
    fn invalid_method() {
        let cls = test_utils::custom_class();
        let mut builder = ClassBuilder::new(&c("TestClassBuilderInvalidMethod"), cls).unwrap();

        extern "C" fn foo(_this: &NSObject, _cmd: Sel) -> i16 {
            0
        }

        unsafe {
            builder.add_method(sel!(foo), foo as extern "C" fn(_, _) -> _);
        }
    }

    #[test]
    #[cfg_attr(
        all(
            debug_assertions,
            not(feature = "disable-encoding-assertions"),
            not(feature = "relax-sign-encoding")
        ),
        should_panic = "defined invalid method +[TestClassBuilderInvalidClassMethod classFoo]: expected return to have type code 'I', but found 'i'"
    )]
    fn invalid_class_method() {
        let cls = test_utils::custom_class();
        let mut builder = ClassBuilder::new(&c("TestClassBuilderInvalidClassMethod"), cls).unwrap();

        extern "C" fn class_foo(_cls: &AnyClass, _cmd: Sel) -> i32 {
            0
        }

        unsafe {
            builder.add_class_method(sel!(classFoo), class_foo as extern "C" fn(_, _) -> _);
        }
    }

    #[test]
    fn inheriting_does_not_implement_protocols() {
        let builder = ClassBuilder::new(
            &c("TestClassBuilderInheritingDoesNotImplementProtocols"),
            NSObject::class(),
        )
        .unwrap();

        let cls = builder.register();
        let conforms = cls.conforms_to(<dyn NSObjectProtocol>::protocol().unwrap());
        if cfg!(any()) {
            // FIXME: GNUStep works differently here!
            assert!(conforms);
        } else {
            assert!(!conforms);
        }
    }

    #[test]
    fn inherit_nsobject_add_protocol() {
        let mut builder = ClassBuilder::new(
            &c("TestClassBuilderInheritNSObjectAddProtocol"),
            NSObject::class(),
        )
        .unwrap();

        let protocol = <dyn NSObjectProtocol>::protocol().unwrap();

        // GNUStep is more eagerly returning false in the case where we
        // inherit something that implements the protocol.
        if cfg!(any()) {
            assert!(!builder.add_protocol(protocol));
        } else {
            assert!(builder.add_protocol(protocol));
        }

        let cls = builder.register();
        assert!(cls.conforms_to(protocol));
    }

    #[test]
    fn duplicate_protocol() {
        let cls = test_utils::custom_class();
        let mut builder = ClassBuilder::new(&c("TestClassBuilderDuplicateProtocol"), cls).unwrap();

        let protocol = ProtocolBuilder::new(&c("TestClassBuilderDuplicateProtocol"))
            .unwrap()
            .register();

        assert!(builder.add_protocol(protocol));
        assert!(!builder.add_protocol(protocol));
    }

    #[test]
    fn add_protocol_subprotocol_ordering() {
        // The value returned by `class_addProtocol` is inherently dependent
        // on the order in which you add the super- and subprotocols.
        let builder = ProtocolBuilder::new(&c("Superprotocol")).unwrap();
        let superprotocol = builder.register();

        let mut builder = ProtocolBuilder::new(&c("Subprotocol")).unwrap();
        builder.add_protocol(superprotocol);
        let subprotocol = builder.register();

        let mut builder =
            ClassBuilder::new(&c("AddProtocolSuperThenSub"), NSObject::class()).unwrap();
        assert!(builder.add_protocol(superprotocol));
        assert!(builder.add_protocol(subprotocol));
        let _cls = builder.register();

        let mut builder =
            ClassBuilder::new(&c("AddProtocolSubThenSuper"), NSObject::class()).unwrap();
        assert!(builder.add_protocol(subprotocol));
        assert!(!builder.add_protocol(superprotocol));
        let _cls = builder.register();
    }

    #[test]
    fn test_classbuilder_drop() {
        let cls = test_utils::custom_class();
        let builder = ClassBuilder::new(&c("TestClassBuilderDrop"), cls).unwrap();
        drop(builder);
        // After we dropped the class, we can create a new one with the same name:
        let _builder = ClassBuilder::new(&c("TestClassBuilderDrop"), cls).unwrap();
    }

    #[test]
    fn test_custom_class() {
        // Registering the custom class is in test_utils
        let obj = test_utils::custom_object();
        let _: () = unsafe { msg_send![&obj, setFoo: 13u32] };
        let result: u32 = unsafe { msg_send![&obj, foo] };
        assert_eq!(result, 13);
    }

    #[test]
    fn test_in_all_classes() {
        fn is_present(cls: *const AnyClass) -> bool {
            // Check whether the class is present in AnyClass::classes()
            AnyClass::classes().iter().any(|item| ptr::eq(cls, *item))
        }

        let superclass = test_utils::custom_class();
        let builder = ClassBuilder::new(&c("TestFetchWhileCreatingClass"), superclass).unwrap();

        if cfg!(all(
            target_vendor = "apple",
            any(target_arch = "aarch64", target_arch = "x86_64")
        )) {
            // It is IMO a bug that it is present here!
            assert!(is_present(builder.cls.as_ptr().cast()));
        } else {
            assert!(!is_present(builder.cls.as_ptr().cast()));
        }

        let cls = builder.register();
        assert!(is_present(cls));
    }

    #[test]
    fn test_class_method() {
        let cls = test_utils::custom_class();
        let result: u32 = unsafe { msg_send![cls, classFoo] };
        assert_eq!(result, 7);
    }

    // Proof-of-concept how we could make define_class! accept generic types.
    #[test]
    fn test_generic() {
        struct GenericDefineClass<T>(T);

        unsafe impl<T> RefEncode for GenericDefineClass<T> {
            const ENCODING_REF: Encoding = Encoding::Object;
        }

        unsafe impl<T> Message for GenericDefineClass<T> {}

        unsafe impl<T> ClassType for GenericDefineClass<T> {
            type Super = NSObject;
            type ThreadKind = <Self::Super as ClassType>::ThreadKind;
            const NAME: &'static str = "GenericDefineClass";

            #[inline]
            fn as_super(&self) -> &Self::Super {
                unimplemented!()
            }

            fn class() -> &'static AnyClass {
                let superclass = NSObject::class();
                let mut builder = ClassBuilder::new(&c(Self::NAME), superclass).unwrap();

                unsafe {
                    builder.add_method(
                        sel!(generic),
                        <GenericDefineClass<T>>::generic as unsafe extern "C" fn(_, _),
                    );
                }

                builder.register()
            }

            const __INNER: () = ();

            type __SubclassingType = Self;
        }

        impl<T> GenericDefineClass<T> {
            extern "C" fn generic(&self, _cmd: Sel) {}
        }

        let _ = GenericDefineClass::<()>::class();
    }

    #[test]
    fn test_inherited_nsobject_methods_work() {
        define_class!(
            #[unsafe(super(NSObject))]
            #[name = "TestInheritedNSObjectMethodsWork"]
            #[derive(Debug, PartialEq, Eq, Hash)]
            struct Custom;
        );

        impl Custom {
            extern_methods!(
                #[unsafe(method(new))]
                fn new() -> Retained<Self>;
            );
        }

        let obj1 = Custom::new();
        let obj2 = Custom::new();

        // isEqual:
        assert_eq!(obj1, obj1);
        assert_ne!(obj1, obj2);

        // description
        let expected =
            format!("Custom {{ super: <TestInheritedNSObjectMethodsWork: {obj1:p}>, ivars: () }}");
        assert_eq!(format!("{obj1:?}"), expected);

        // hash
        let mut hashstate1 = DefaultHasher::new();
        let mut hashstate2 = DefaultHasher::new();

        obj1.hash(&mut hashstate1);
        obj1.hash(&mut hashstate2);

        assert_eq!(hashstate1.finish(), hashstate2.finish());

        let mut hashstate2 = DefaultHasher::new();
        obj2.hash(&mut hashstate2);
        assert_ne!(hashstate1.finish(), hashstate2.finish());

        // isKindOfClass:
        assert!(obj1.isKindOfClass(NSObject::class()));
        assert!(obj1.isKindOfClass(Custom::class()));
        assert!((**obj1).isKindOfClass(Custom::class()));
    }

    #[test]
    #[cfg_attr(any(), ignore = "ivars cannot have the same name on GNUStep")]
    fn test_ivar_sizing() {
        #[repr(align(16))]
        struct U128align16 {
            _inner: [u64; 2],
        }

        unsafe impl Encode for U128align16 {
            const ENCODING: Encoding = <[u64; 2]>::ENCODING;
        }

        let mut superclass =
            ClassBuilder::new(&c("DefineClassDuplicateIvarSuperclass"), NSObject::class()).unwrap();
        superclass.add_ivar::<u8>(&c("ivar1"));
        superclass.add_ivar::<U128align16>(&c("ivar2"));
        superclass.add_ivar::<u8>(&c("ivar3"));
        superclass.add_ivar::<[u8; 0]>(&c("ivar4"));
        let superclass = superclass.register();

        let mut subclass =
            ClassBuilder::new(&c("DefineClassDuplicateIvarSubclass"), superclass).unwrap();
        // Try to overwrite instance variables
        subclass.add_ivar::<i16>(&c("ivar1"));
        subclass.add_ivar::<usize>(&c("ivar2"));
        subclass.add_ivar::<*const AnyObject>(&c("ivar3"));
        subclass.add_ivar::<usize>(&c("ivar4"));
        let subclass = subclass.register();

        // Test that ivar layout matches that of C
        //
        // In particular, ivars are not reordered, though any extra padding on
        // superclasses are utilized on subclasses.
        #[repr(C)]
        struct NSObjectLayout {
            isa: *const AnyClass,
        }
        assert_eq!(
            NSObject::class().instance_size(),
            mem::size_of::<NSObjectLayout>(),
        );

        #[repr(C)]
        struct SuperLayout {
            isa: *const AnyClass,
            ivar1: u8,
            // Padding (7 on 64bit, 11 on 32bit)
            ivar2: U128align16,
            ivar3: u8,
            ivar4: [u8; 0],
            // Padding (15 in Rust, 7 on 64bit, 3 on 32bit)
        }
        // Class's ivar size is only rounded up to a pointer-sized boundary,
        // not all the way up to the maximum alignment.
        //
        // This is surprising, but actually fine, since Objective-C objects
        // are never packed closely like Rust structs would be in an array.
        assert_eq!(
            superclass.instance_size(),
            mem::size_of::<SuperLayout>() - 16 + mem::size_of::<*const AnyClass>(),
        );

        #[repr(C)]
        struct SubLayout {
            isa: *const AnyClass,
            ivar1: u8,
            // Padding (7 on 64bit, 11 on 32bit)
            ivar2: U128align16,
            ivar3: u8,
            ivar4: [u8; 0],
            // Padding (1)
            ivar1_b: i16,
            // Padding (4)
            ivar2_b: usize,
            ivar3_b: *const AnyObject,
            ivar4_b: usize,
        }
        assert_eq!(subclass.instance_size(), mem::size_of::<SubLayout>());

        let superclass_ivar1 = superclass.instance_variable(&c("ivar1")).unwrap();
        let superclass_ivar2 = superclass.instance_variable(&c("ivar2")).unwrap();
        let superclass_ivar3 = superclass.instance_variable(&c("ivar3")).unwrap();
        let superclass_ivar4 = superclass.instance_variable(&c("ivar4")).unwrap();
        let subclass_ivar1 = subclass.instance_variable(&c("ivar1")).unwrap();
        let subclass_ivar2 = subclass.instance_variable(&c("ivar2")).unwrap();
        let subclass_ivar3 = subclass.instance_variable(&c("ivar3")).unwrap();
        let subclass_ivar4 = subclass.instance_variable(&c("ivar4")).unwrap();

        // Ensure that duplicate names do not conflict
        assert_ne!(superclass_ivar1, subclass_ivar1);
        assert_ne!(superclass_ivar2, subclass_ivar2);
        assert_ne!(superclass_ivar3, subclass_ivar3);
        assert_ne!(superclass_ivar4, subclass_ivar4);

        // Ensure that all offsets are as expected
        assert_eq!(
            superclass_ivar1.offset(),
            offset_of!(SuperLayout, ivar1) as isize
        );
        assert_eq!(
            superclass_ivar2.offset(),
            offset_of!(SuperLayout, ivar2) as isize
        );
        assert_eq!(
            superclass_ivar3.offset(),
            offset_of!(SuperLayout, ivar3) as isize
        );
        assert_eq!(
            superclass_ivar4.offset(),
            offset_of!(SuperLayout, ivar4) as isize
        );
        assert_eq!(
            subclass_ivar1.offset(),
            offset_of!(SubLayout, ivar1_b) as isize
        );
        assert_eq!(
            subclass_ivar2.offset(),
            offset_of!(SubLayout, ivar2_b) as isize
        );
        assert_eq!(
            subclass_ivar3.offset(),
            offset_of!(SubLayout, ivar3_b) as isize
        );
        assert_eq!(
            subclass_ivar4.offset(),
            offset_of!(SubLayout, ivar4_b) as isize
        );

        // Ensure our ivar loading works correctly
        let obj: Retained<NSObject> = unsafe { msg_send![subclass, new] };
        let ptr = unsafe { *subclass_ivar3.load::<*const AnyObject>(&obj) };
        assert!(ptr.is_null());

        // Illustration of what goes wrong with the naive approach of loading
        // the Ivar dynamically; in short, we can't be sure of which instance
        // variable we're referring to here.
        //
        // let ivar = *obj.get_ivar::<u8>("ivar3");
    }

    #[test]
    fn auto_name() {
        define_class!(
            #[unsafe(super(NSObject))]
            #[ivars = ()]
            struct AutoName;
        );

        let expected = format!(
            "objc2::runtime::define::tests::AutoName{}",
            env!("CARGO_PKG_VERSION")
        );

        let cls = AutoName::class();
        assert_eq!(cls.name().to_str().unwrap(), expected);
        assert_eq!(AutoName::NAME, expected);
    }

