
    use super::*;

    use core::any::TypeId;
    use core::ptr;

    use crate::sel;

    #[test]
    fn convert_normally_noop() {
        assert_eq!(
            TypeId::of::<<i32 as ConvertArgument>::__Inner>(),
            TypeId::of::<i32>()
        );
        assert_eq!(<i32 as ConvertArgument>::__from_defined_param(42), 42);
        assert_eq!(unsafe { ConvertArgument::__into_argument(42i32).0 }, 42);
    }

    #[test]
    fn convert_i8() {
        assert_eq!(
            TypeId::of::<<i8 as ConvertArgument>::__Inner>(),
            TypeId::of::<i8>()
        );
        assert_eq!(<i8 as ConvertArgument>::__from_defined_param(-3), -3);
        assert_eq!(unsafe { ConvertArgument::__into_argument(-3i32).0 }, -3);
    }

    #[test]
    fn convert_bool() {
        let receiver_ptr = ptr::null_mut::<AnyObject>();
        let sel = sel!(foo);

        assert!(!<bool as ConvertArgument>::__from_defined_param(Bool::NO));
        assert!(<bool as ConvertArgument>::__from_defined_param(Bool::YES));
        assert!(!unsafe {
            <bool as ConvertReturn<()>>::convert_message_return(Bool::NO, receiver_ptr, sel)
        });
        assert!(unsafe {
            <bool as ConvertReturn<()>>::convert_message_return(Bool::YES, receiver_ptr, sel)
        });

        assert!(!unsafe { ConvertArgument::__into_argument(false).0 }.as_bool());
        assert!(unsafe { ConvertArgument::__into_argument(true).0 }.as_bool());
        assert!(!ConvertReturn::<()>::convert_defined_return(false).as_bool());
        assert!(ConvertReturn::<()>::convert_defined_return(true).as_bool());

        #[cfg(all(target_vendor = "apple", target_os = "macos", target_arch = "x86_64"))]
        assert_eq!(
            <bool as ConvertArgument>::__Inner::ENCODING_ARGUMENT,
            crate::encode::Encoding::Char,
        );
    }

