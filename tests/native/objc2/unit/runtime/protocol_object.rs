
    use alloc::format;
    use core::ffi::CStr;

    use static_assertions::{assert_impl_all, assert_not_impl_any};

    use super::*;
    use crate::runtime::{ClassBuilder, NSObject};
    use crate::{define_class, extern_methods, extern_protocol, msg_send, ClassType};

    extern_protocol!(
        unsafe trait Foo {
            #[unsafe(method(foo))]
            fn foo_class();

            #[unsafe(method(foo))]
            fn foo_instance(&self);
        }
    );

    extern_protocol!(
        unsafe trait Bar: NSObjectProtocol {
            #[unsafe(method(bar))]
            fn bar_class();

            #[unsafe(method(bar))]
            fn bar_instance(&self);
        }
    );

    extern_protocol!(
        unsafe trait FooBar: Foo + Bar {
            #[unsafe(method(foobar))]
            fn foobar_class();

            #[unsafe(method(foobar))]
            fn foobar_instance(&self);
        }
    );

    extern_protocol!(
        unsafe trait FooFooBar: Foo + FooBar {
            #[unsafe(method(foofoobar))]
            fn foofoobar_class();

            #[unsafe(method(foofoobar))]
            fn foofoobar_instance(&self);
        }
    );

    define_class!(
        #[unsafe(super(NSObject))]
        #[derive(Debug, PartialEq, Eq, Hash)]
        struct DummyClass;

        unsafe impl NSObjectProtocol for DummyClass {}
    );

    unsafe impl Foo for DummyClass {}
    unsafe impl Bar for DummyClass {}
    unsafe impl FooBar for DummyClass {}
    // unsafe impl FooFooBar for DummyClass {}

    impl DummyClass {
        extern_methods!(
            #[unsafe(method(new))]
            fn new() -> Retained<Self>;
        );
    }

    #[test]
    fn impl_traits() {
        assert_impl_all!(NSObject: NSObjectProtocol);
        assert_impl_all!(ProtocolObject<dyn NSObjectProtocol>: NSObjectProtocol);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol>: Send, Sync);
        assert_impl_all!(ProtocolObject<dyn NSObjectProtocol + Send>: NSObjectProtocol, Send);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol + Send>: Sync);
        assert_impl_all!(ProtocolObject<dyn NSObjectProtocol + Sync>: NSObjectProtocol, Sync);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol + Sync>: Send);
        assert_impl_all!(ProtocolObject<dyn NSObjectProtocol + Send + Sync>: NSObjectProtocol, Send, Sync);
        assert_not_impl_any!(ProtocolObject<dyn Foo>: NSObjectProtocol);
        assert_impl_all!(ProtocolObject<dyn Bar>: NSObjectProtocol);
        assert_impl_all!(ProtocolObject<dyn FooBar>: NSObjectProtocol);
        assert_impl_all!(ProtocolObject<dyn FooFooBar>: NSObjectProtocol);
        assert_impl_all!(DummyClass: NSObjectProtocol);

        assert_not_impl_any!(NSObject: Foo);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol>: Foo);
        assert_impl_all!(ProtocolObject<dyn Foo>: Foo);
        assert_not_impl_any!(ProtocolObject<dyn Bar>: Foo);
        assert_impl_all!(ProtocolObject<dyn FooBar>: Foo);
        assert_impl_all!(ProtocolObject<dyn FooFooBar>: Foo);
        assert_impl_all!(DummyClass: Foo);

        assert_not_impl_any!(NSObject: Bar);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol>: Bar);
        assert_not_impl_any!(ProtocolObject<dyn Foo>: Bar);
        assert_impl_all!(ProtocolObject<dyn Bar>: Bar);
        assert_impl_all!(ProtocolObject<dyn FooBar>: Bar);
        assert_impl_all!(ProtocolObject<dyn FooFooBar>: Bar);
        assert_impl_all!(DummyClass: Bar);

        assert_not_impl_any!(NSObject: FooBar);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol>: FooBar);
        assert_not_impl_any!(ProtocolObject<dyn Foo>: FooBar);
        assert_not_impl_any!(ProtocolObject<dyn Bar>: FooBar);
        assert_impl_all!(ProtocolObject<dyn FooBar>: FooBar);
        assert_impl_all!(ProtocolObject<dyn FooFooBar>: FooBar);
        assert_impl_all!(DummyClass: FooBar);

        assert_not_impl_any!(NSObject: FooFooBar);
        assert_not_impl_any!(ProtocolObject<dyn NSObjectProtocol>: FooFooBar);
        assert_not_impl_any!(ProtocolObject<dyn Foo>: FooFooBar);
        assert_not_impl_any!(ProtocolObject<dyn Bar>: FooFooBar);
        assert_not_impl_any!(ProtocolObject<dyn FooBar>: FooFooBar);
        assert_impl_all!(ProtocolObject<dyn FooFooBar>: FooFooBar);
        assert_not_impl_any!(DummyClass: FooFooBar);
    }

    #[test]
    fn convertible() {
        let obj = DummyClass::new();
        let foobar: &ProtocolObject<dyn FooBar> = ProtocolObject::from_ref(&*obj);
        let foobar: &ProtocolObject<dyn FooBar> = ProtocolObject::from_ref(foobar);

        let _bar: &ProtocolObject<dyn Bar> = ProtocolObject::from_ref(foobar);
        let bar: &ProtocolObject<dyn Bar> = ProtocolObject::from_ref(&*obj);
        let bar: &ProtocolObject<dyn Bar> = ProtocolObject::from_ref(bar);

        let _foo: &ProtocolObject<dyn Foo> = ProtocolObject::from_ref(foobar);
        let foo: &ProtocolObject<dyn Foo> = ProtocolObject::from_ref(&*obj);
        let _foo: &ProtocolObject<dyn Foo> = ProtocolObject::from_ref(foo);

        let _nsobject: &ProtocolObject<dyn NSObjectProtocol> = ProtocolObject::from_ref(foobar);
        let _nsobject: &ProtocolObject<dyn NSObjectProtocol> = ProtocolObject::from_ref(bar);
        let nsobject: &ProtocolObject<dyn NSObjectProtocol> = ProtocolObject::from_ref(&*obj);
        let _nsobject: &ProtocolObject<dyn NSObjectProtocol> = ProtocolObject::from_ref(nsobject);
        let _: &ProtocolObject<dyn NSObjectProtocol + Send> = ProtocolObject::from_ref(&*obj);
        let _: &ProtocolObject<dyn NSObjectProtocol + Sync> = ProtocolObject::from_ref(&*obj);
        let _: &ProtocolObject<dyn NSObjectProtocol + Send + Sync> =
            ProtocolObject::from_ref(&*obj);

        let _foobar: Retained<ProtocolObject<dyn FooBar>> = ProtocolObject::from_retained(obj);
    }

    #[test]
    fn convert_to_anyobj() {
        let obj = NSObject::new();
        let obj: Retained<ProtocolObject<dyn NSObjectProtocol>> =
            ProtocolObject::from_retained(obj);
        let _obj: &AnyObject = obj.as_ref();
    }

    #[test]
    fn test_traits() {
        use core::hash::Hasher;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;

        let obj = DummyClass::new();
        let obj2 = DummyClass::new();

        let foobar: &ProtocolObject<dyn FooBar> = ProtocolObject::from_ref(&*obj);
        let foobar2: &ProtocolObject<dyn FooBar> = ProtocolObject::from_ref(&*obj2);

        assert_eq!(
            format!("{obj:?}"),
            format!("DummyClass {{ super: {foobar:?}, ivars: () }}"),
        );
        assert_eq!(obj == obj2, foobar == foobar2);

        let mut hashstate_a = DefaultHasher::new();
        let mut hashstate_b = DefaultHasher::new();

        obj.hash(&mut hashstate_a);
        <_ as Hash>::hash(foobar, &mut hashstate_b);

        assert_eq!(hashstate_a.finish(), hashstate_b.finish());
    }

    // We use `debug_assertions` here just because it's something that we know
    // our CI already tests.
    extern_protocol!(
        #[cfg(debug_assertions)]
        unsafe trait CfgTest {}
    );

    #[test]
    #[cfg(debug_assertions)]
    fn test_meta() {
        if false {
            let _protocol = <dyn CfgTest as crate::ProtocolType>::protocol();
        }
    }

    #[test]
    #[cfg_attr(any(), ignore = "depends on the platform's NSString unicode handling")]
    fn debug_non_utf8_classname() {
        // Some class with invalid UTF-8 character inside
        let s = CStr::from_bytes_with_nul(b"My\xF0\x90\x80Class\0").unwrap();

        let cls = ClassBuilder::new(s, NSObject::class()).unwrap().register();
        let obj: Retained<NSObject> = unsafe { msg_send![cls, new] };

        let expected = format!("<My\u{f8ff}êÄClass: {:p}>", &*obj);
        assert_eq!(format!("{obj:?}"), expected);
    }

