
    use alloc::ffi::CString;
    use alloc::format;
    use alloc::string::ToString;
    use core::mem::size_of;

    use super::*;
    use crate::test_utils;
    use crate::{class, msg_send, sel, ClassType, ProtocolType};

    // TODO: Remove once c"" strings are in MSRV
    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn test_selector() {
        macro_rules! test_sel {
            ($s:literal, $($tt:tt)+) => {{
                let sel = sel!($($tt)*);
                let expected = Sel::register(&c($s));
                assert_eq!(sel, expected);
                assert_eq!(sel.name().to_str(), Ok($s));
            }}
        }
        test_sel!("abc", abc);
        test_sel!("abc:", abc:);
        test_sel!("abc:def:", abc:def:);
        test_sel!("abc:def:ghi:", abc:def:ghi:);
        test_sel!("functionWithControlPoints::::", functionWithControlPoints::::);
        test_sel!("initWithControlPoints::::", initWithControlPoints::::);
        test_sel!("setFloatValue::", setFloatValue::);
        test_sel!("isSupported::", isSupported::);
        test_sel!("addEventListener:::", addEventListener:::);
        test_sel!("test::arg::", test::arg::);
        test_sel!("test::::with::spaces::", test : :: : with : : spaces : :);
        test_sel!("a::b:", a::b:);
    }

    #[test]
    fn test_empty_selector() {
        let s = c("");
        let sel = Sel::register(&s);
        assert_eq!(sel.name(), &*s);
        let s = c(":");
        let sel = Sel::register(&s);
        assert_eq!(sel.name(), &*s);
        let s = c("::");
        let sel = Sel::register(&s);
        assert_eq!(sel.name(), &*s);
    }

    #[test]
    fn test_ivar() {
        let cls = test_utils::custom_class();
        let ivar = cls.instance_variable(&c("_foo")).unwrap();
        assert_eq!(ivar.name(), &*c("_foo"));
        assert!(<u32>::ENCODING.equivalent_to_str(ivar.type_encoding().to_str().unwrap()));
        assert!(ivar.offset() > 0);
        assert!(cls.instance_variables().len() > 0);
    }

    #[test]
    fn test_instance_method() {
        let cls = test_utils::custom_class();
        let sel = Sel::register(&c("foo"));
        let method = cls.instance_method(sel).unwrap();
        assert_eq!(method.name().name(), &*c("foo"));
        assert_eq!(method.arguments_count(), 2);

        assert!(<u32>::ENCODING.equivalent_to_str(method.return_type().to_str().unwrap()));
        assert!(Sel::ENCODING.equivalent_to_str(method.argument_type(1).unwrap().to_str().unwrap()));

        assert!(cls.instance_methods().contains(&method));
    }

    #[test]
    fn test_class_method() {
        let cls = test_utils::custom_class();
        let method = cls.class_method(sel!(classFoo)).unwrap();
        assert_eq!(method.name().name(), &*c("classFoo"));
        assert_eq!(method.arguments_count(), 2);

        assert!(<u32>::ENCODING.equivalent_to_str(method.return_type().to_str().unwrap()));
        assert!(Sel::ENCODING.equivalent_to_str(method.argument_type(1).unwrap().to_str().unwrap()));

        assert!(cls.metaclass().instance_methods().contains(&method));
    }

    #[test]
    fn test_class() {
        let cls = test_utils::custom_class();
        assert_eq!(cls.name(), &*c("CustomObject"));
        assert!(cls.instance_size() > 0);
        assert!(cls.superclass().is_none());

        assert!(cls.responds_to(sel!(foo)));
        assert!(cls.responds_to(sel!(setBar:)));
        assert!(cls.responds_to(sel!(test::test::)));
        assert!(!cls.responds_to(sel!(abc)));
        assert!(!cls.responds_to(sel!(addNumber:toNumber:)));

        assert_eq!(AnyClass::get(cls.name()), Some(cls));

        let metaclass = cls.metaclass();
        // The metaclass of a root class is a subclass of the root class
        assert_eq!(metaclass.superclass().unwrap(), cls);
        assert!(metaclass.responds_to(sel!(addNumber:toNumber:)));
        assert!(metaclass.responds_to(sel!(test::test::)));
        // TODO: This is unexpected!
        assert!(metaclass.responds_to(sel!(foo)));

        let subclass = test_utils::custom_subclass();
        assert_eq!(subclass.superclass().unwrap(), cls);
    }

    #[test]
    fn test_classes_count() {
        assert!(AnyClass::classes_count() > 0);
    }

    #[test]
    fn test_classes() {
        let classes = AnyClass::classes();
        assert!(classes.len() > 0);
    }

    #[test]
    fn test_protocol() {
        let proto = test_utils::custom_protocol();
        assert_eq!(proto.name(), &*c("CustomProtocol"));
        let class = test_utils::custom_class();
        assert!(class.conforms_to(proto));

        // The selectors are broken somehow on GNUStep < 2.0
        if cfg!(any(not(any()), feature = "gnustep-2-0")) {
            let desc = MethodDescription {
                sel: sel!(setBar:),
                types: CStr::from_bytes_with_nul(b"v@:i\0").unwrap(),
            };
            assert_eq!(&proto.method_descriptions(true), &[desc]);
            let desc = MethodDescription {
                sel: sel!(getName),
                types: CStr::from_bytes_with_nul(b"*@:\0").unwrap(),
            };
            assert_eq!(&proto.method_descriptions(false), &[desc]);
            let desc = MethodDescription {
                sel: sel!(addNumber:toNumber:),
                types: CStr::from_bytes_with_nul(b"i@:ii\0").unwrap(),
            };
            assert_eq!(&proto.class_method_descriptions(true), &[desc]);
        }
        assert_eq!(&proto.class_method_descriptions(false), &[]);

        assert!(class.adopted_protocols().contains(&proto));
    }

    #[test]
    fn test_protocol_method() {
        let class = test_utils::custom_class();
        let result: i32 = unsafe { msg_send![class, addNumber: 1, toNumber: 2] };
        assert_eq!(result, 3);
    }

    #[test]
    fn class_self() {
        let cls = NSObject::class();
        let result: &'static AnyClass = unsafe { msg_send![cls, self] };
        assert_eq!(cls, result);
    }

    #[test]
    fn test_subprotocols() {
        let sub_proto = test_utils::custom_subprotocol();
        let super_proto = test_utils::custom_protocol();
        assert!(sub_proto.conforms_to(super_proto));
        assert_eq!(sub_proto.adopted_protocols()[0], super_proto);
    }

    #[test]
    fn test_protocols() {
        // Ensure that a protocol has been registered on linux
        let _ = test_utils::custom_protocol();

        assert!(AnyProtocol::protocols().len() > 0);
    }

    #[test]
    fn test_object() {
        let obj = test_utils::custom_object();
        let cls = test_utils::custom_class();
        assert_eq!(obj.class(), cls);

        let ivar = cls.instance_variable(&c("_foo")).unwrap();

        unsafe { *ivar.load_ptr::<u32>(&obj) = 4 };
        let result = unsafe { *ivar.load::<u32>(&obj) };
        assert_eq!(result, 4);
    }

    #[test]
    fn test_object_ivar_unknown() {
        let cls = test_utils::custom_class();
        assert_eq!(cls.instance_variable(&c("unknown")), None);
    }

    #[test]
    fn test_no_ivars() {
        let cls = ClassBuilder::new(&c("NoIvarObject"), NSObject::class())
            .unwrap()
            .register();
        assert_eq!(cls.instance_variables().len(), 0);
    }

    #[test]
    #[cfg_attr(
        all(debug_assertions, not(any())),
        should_panic = "wrong encoding. Tried to retrieve ivar with encoding I, but the encoding of the given type was C"
    )]
    fn test_object_ivar_wrong_type() {
        let obj = test_utils::custom_object();
        let cls = test_utils::custom_class();
        let ivar = cls.instance_variable(&c("_foo")).unwrap();
        let _ = unsafe { *ivar.load::<u8>(&obj) };
    }

    #[test]
    fn test_encode() {
        fn assert_enc<T: Encode>(expected: &str) {
            assert_eq!(&T::ENCODING.to_string(), expected);
        }
        assert_enc::<&AnyObject>("@");
        assert_enc::<*mut AnyObject>("@");
        assert_enc::<&AnyClass>("#");
        assert_enc::<Sel>(":");
        assert_enc::<Option<Sel>>(":");
        assert_enc::<Imp>("^?");
        assert_enc::<Option<Imp>>("^?");
        assert_enc::<&AnyProtocol>("@");
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<Bool>();
        assert_send_sync::<AnyClass>();
        assert_send_sync::<Ivar>();
        assert_send_sync::<Method>();
        assert_send_sync::<AnyProtocol>();
        assert_send_sync::<Sel>();
    }

    #[test]
    fn test_debug_display() {
        let sel = sel!(abc:);
        assert_eq!(format!("{sel}"), "abc:");
        assert_eq!(format!("{sel:?}"), "Sel(\"abc:\")");
        let cls = test_utils::custom_class();
        assert_eq!(format!("{cls}"), "CustomObject");
        assert_eq!(
            format!("{cls:?}"),
            "AnyClass { name: \"CustomObject\", .. }"
        );
        let protocol = test_utils::custom_protocol();
        assert_eq!(format!("{protocol}"), "CustomProtocol");
        assert_eq!(
            format!("{protocol:?}"),
            "AnyProtocol { name: \"CustomProtocol\", .. }"
        );

        let object = test_utils::custom_object();
        assert_eq!(
            format!("{:?}", &*object),
            format!("CustomObject(<CustomObject: {:p}>)", &*object)
        );
    }

    #[test]
    fn test_multiple_colon() {
        let class = test_utils::custom_class();
        let res: i32 = unsafe {
            MessageReceiver::send_message(class, sel!(test::test::), (1i32, 2i32, 3i32, 4i32))
        };
        assert_eq!(res, 10);

        let obj = test_utils::custom_object();
        let res: i32 = unsafe {
            MessageReceiver::send_message(&*obj, sel!(test::test::), (1i32, 2i32, 3i32, 4i32))
        };
        assert_eq!(res, 24);
    }

    #[test]
    fn test_sizes() {
        assert_eq!(size_of::<Sel>(), size_of::<*const ()>());
        assert_eq!(size_of::<Sel>(), size_of::<Option<Sel>>());

        // These must be zero-sized until we get extern types, otherwise the
        // optimizer may invalidly assume something about their layout.
        assert_eq!(size_of::<AnyClass>(), 0);
        assert_eq!(size_of::<AnyObject>(), 0);
        assert_eq!(size_of::<AnyProtocol>(), 0);
        assert_eq!(size_of::<Ivar>(), 0);
        assert_eq!(size_of::<Method>(), 0);
    }

    fn get_ivar_layout(cls: &AnyClass) -> *const u8 {
        unsafe { ffi::class_getIvarLayout(cls) }
    }

    #[test]
    #[cfg_attr(any(), ignore = "ivar layout is still used on GNUStep")]
    fn test_layout_does_not_matter_any_longer() {
        assert!(get_ivar_layout(class!(NSObject)).is_null());
        assert!(get_ivar_layout(class!(NSArray)).is_null());
        assert!(get_ivar_layout(class!(NSException)).is_null());
        assert!(get_ivar_layout(class!(NSNumber)).is_null());
        assert!(get_ivar_layout(class!(NSString)).is_null());
    }

    #[test]
    fn test_non_utf8_roundtrip() {
        // Some invalid UTF-8 character
        let s = CStr::from_bytes_with_nul(b"\x9F\0").unwrap();

        let sel = Sel::register(s);
        assert_eq!(sel.name(), s);
        assert_eq!(sel.to_string(), char::REPLACEMENT_CHARACTER.to_string());

        let cls = ClassBuilder::new(s, NSObject::class()).unwrap().register();
        assert_eq!(cls.name(), s);
        assert_eq!(cls.to_string(), char::REPLACEMENT_CHARACTER.to_string());

        let cls_runtime = AnyClass::get(s).unwrap();
        assert_eq!(cls, cls_runtime);
    }

    #[test]
    fn class_is_object() {
        let cls = NSObject::class();
        let retained = cls.retain();
        assert_eq!(&*retained, cls);

        let obj: &AnyObject = cls.as_ref();
        let superclass = obj.class();
        assert!(superclass.conforms_to(<dyn NSObjectProtocol>::protocol().unwrap()));

        // Classes are NSObject subclasses in the current runtime.
        let ns_obj = retained.downcast::<NSObject>().unwrap();
        // Test that we can call NSObject methods on classes.
        assert_eq!(ns_obj, ns_obj);
        let _ = ns_obj.retainCount();
    }

    #[test]
    fn class_has_infinite_retain_count() {
        let obj: &AnyObject = NSObject::class().as_ref();
        let obj = obj.downcast_ref::<NSObject>().unwrap();

        let large_retain = if cfg!(any()) {
            u32::MAX as usize
        } else {
            usize::MAX
        };

        assert_eq!(obj.retainCount(), large_retain);
        let obj2 = obj.retain();
        assert_eq!(obj.retainCount(), large_retain);
        drop(obj2);
        assert_eq!(obj.retainCount(), large_retain);
    }

    #[test]
    fn protocol_is_object() {
        let protocol = <dyn NSObjectProtocol>::protocol().unwrap();
        let retained = protocol.retain();
        assert_eq!(&*retained, protocol);

        // Protocols don't implement isKindOfClass: on GNUStep.
        if cfg!(any()) {
            return;
        }

        // In the old runtime, NSObjectProtocol are not NSObject subclasses.
        if cfg!(all(target_os = "macos", target_arch = "x86")) {
            let _ = retained.downcast::<NSObject>().unwrap_err();
        } else {
            // But elsewhere they are.
            let obj = retained.downcast::<NSObject>().unwrap();
            // Test that we can call NSObject methods on protocols.
            assert_eq!(obj, obj);
            let _ = obj.retainCount();
        }
    }

