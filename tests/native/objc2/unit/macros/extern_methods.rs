
    use crate::extern_methods;
    use crate::runtime::{NSObject, NSObjectProtocol};

    #[test]
    fn outside_impl_using_this() {
        // The fact that this works outside `impl` is an implementation detail
        // that will get resolved once we have arbitrary self types.
        extern_methods!(
            #[unsafe(method(hash))]
            fn obj_hash(this: &NSObject) -> usize;
        );

        let obj = NSObject::new();
        assert_eq!(obj_hash(&obj), obj.hash())
    }

