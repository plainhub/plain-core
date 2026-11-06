
    use super::*;

    use core::sync::atomic::*;

    #[test]
    fn test_c_string() {
        assert_eq!(i8::ENCODING, Encoding::Char);
        assert_eq!(u8::ENCODING, Encoding::UChar);

        assert_eq!(<*const i8>::ENCODING, Encoding::String);
        assert_eq!(<&u8>::ENCODING, Encoding::String);
        assert_eq!(i8::ENCODING_REF, Encoding::String);
        assert_eq!(i8::ENCODING_REF, Encoding::String);

        assert_eq!(
            <*const *const i8>::ENCODING,
            Encoding::Pointer(&Encoding::String)
        );
        assert_eq!(<&&u8>::ENCODING, Encoding::Pointer(&Encoding::String));
    }

    #[test]
    fn test_i32() {
        assert_eq!(i32::ENCODING, Encoding::Int);
        assert_eq!(<&i32>::ENCODING, Encoding::Pointer(&Encoding::Int));
        assert_eq!(
            <&&i32>::ENCODING,
            Encoding::Pointer(&Encoding::Pointer(&Encoding::Int))
        );
    }

    #[test]
    fn test_atomic() {
        assert_eq!(AtomicI32::ENCODING, Encoding::Atomic(&Encoding::Int));
        assert_eq!(
            AtomicI32::ENCODING_REF,
            Encoding::Pointer(&Encoding::Atomic(&Encoding::Int))
        );
        assert_eq!(
            AtomicPtr::<i32>::ENCODING,
            Encoding::Atomic(&Encoding::Pointer(&Encoding::Int))
        );

        assert_eq!(AtomicI8::ENCODING, Encoding::Atomic(&Encoding::Char));
        assert_eq!(
            AtomicI8::ENCODING_REF,
            Encoding::Pointer(&Encoding::Atomic(&Encoding::Char))
        );
        assert_eq!(
            AtomicPtr::<i8>::ENCODING,
            Encoding::Atomic(&Encoding::String)
        );
    }

    #[test]
    fn test_void() {
        assert_eq!(
            <*const c_void>::ENCODING,
            Encoding::Pointer(&Encoding::Void)
        );
        assert_eq!(<&c_void>::ENCODING, Encoding::Pointer(&Encoding::Void));
        assert_eq!(
            <&*const c_void>::ENCODING,
            Encoding::Pointer(&Encoding::Pointer(&Encoding::Void))
        );
        assert_eq!(
            <AtomicPtr<c_void>>::ENCODING,
            Encoding::Atomic(&Encoding::Pointer(&Encoding::Void))
        );
    }

    #[test]
    fn test_transparent() {
        assert_eq!(<ManuallyDrop<u8>>::ENCODING, u8::ENCODING);
        assert_eq!(<ManuallyDrop<&u8>>::ENCODING, u8::ENCODING_REF);
        assert_eq!(<ManuallyDrop<Option<&u8>>>::ENCODING, u8::ENCODING_REF);
        assert_eq!(<&ManuallyDrop<Option<&u8>>>::ENCODING, <&&u8>::ENCODING);

        assert_eq!(<UnsafeCell<u8>>::ENCODING, u8::ENCODING);
        assert_eq!(<UnsafeCell<&u8>>::ENCODING, <&u8>::ENCODING);
        assert_eq!(<Cell<u8>>::ENCODING, u8::ENCODING);
        assert_eq!(<Cell<&u8>>::ENCODING, <&u8>::ENCODING);
        // assert_eq!(<Pin<u8>>::ENCODING, u8::ENCODING);
        assert_eq!(<MaybeUninit<u8>>::ENCODING, u8::ENCODING);
        assert_eq!(<Wrapping<u8>>::ENCODING, u8::ENCODING);
    }

    #[test]
    fn test_extern_fn_pointer() {
        assert_eq!(
            <extern "C" fn()>::ENCODING,
            Encoding::Pointer(&Encoding::Unknown)
        );
        assert_eq!(
            <extern "C" fn(x: i32) -> u32>::ENCODING,
            Encoding::Pointer(&Encoding::Unknown)
        );
        assert_eq!(
            <Option<unsafe extern "C" fn()>>::ENCODING,
            Encoding::Pointer(&Encoding::Unknown)
        );
        assert_eq!(
            <extern "C-unwind" fn()>::ENCODING,
            Encoding::Pointer(&Encoding::Unknown)
        );
    }

    #[test]
    fn test_extern_fn_pointer_elided_lifetime() {
        fn impls_encode<T: Encode>(_x: T) {}

        extern "C" fn my_fn1(_x: &i32) {}
        extern "C" fn my_fn2(_x: &i32, _y: &u8) {}
        extern "C" fn my_fn3(x: &u8) -> &u8 {
            x
        }
        extern "C" fn my_fn4<'a>(x: &'a u8, _y: &i32) -> &'a u8 {
            x
        }

        impls_encode(my_fn1 as extern "C" fn(_));
        impls_encode(my_fn2 as extern "C" fn(_, _));
        impls_encode(my_fn3 as extern "C" fn(_) -> _);
        impls_encode(my_fn4 as extern "C" fn(_, _) -> _);
    }

    #[test]
    fn test_return() {
        assert_eq!(<i32>::ENCODING_RETURN, <i32>::ENCODING);
        assert_eq!(<()>::ENCODING_RETURN, Encoding::Void);
    }

    #[test]
    fn test_argument() {
        assert_eq!(<i32>::ENCODING_ARGUMENT, <i32>::ENCODING);
    }

    #[test]
    fn test_arguments() {
        assert_eq!(<()>::ENCODINGS, &[] as &[Encoding]);
        assert_eq!(<(i8,)>::ENCODINGS, &[i8::ENCODING]);
        assert_eq!(<(i8, u32)>::ENCODINGS, &[i8::ENCODING, u32::ENCODING]);
    }

