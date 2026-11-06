
    use super::*;
    use crate::__macro_helpers::{ConvertArgument, ConvertReturn};
    use alloc::format;

    #[test]
    fn test_basic() {
        let b = Bool::new(true);
        assert!(b.as_bool());
        assert!(b.is_true());
        assert!(!b.is_false());
        assert!(bool::from(b));
        assert_eq!(b.as_raw() as usize, 1);

        let b = Bool::new(false);
        assert!(!b.as_bool());
        assert!(!b.is_true());
        assert!(b.is_false());
        assert!(!bool::from(b));
        assert_eq!(b.as_raw() as usize, 0);
    }

    #[test]
    fn test_associated_constants() {
        let b = Bool::YES;
        assert!(b.as_bool());
        assert!(b.is_true());
        assert_eq!(b.as_raw() as usize, 1);

        let b = Bool::NO;
        assert!(!b.as_bool());
        assert!(b.is_false());
        assert_eq!(b.as_raw() as usize, 0);
    }

    #[test]
    fn test_encode() {
        assert_eq!(bool::__ENCODING, Encoding::Bool);

        assert_eq!(
            <bool as ConvertArgument>::__Inner::__ENCODING,
            <bool as ConvertArgument>::__Inner::ENCODING
        );
        assert_eq!(
            <bool as ConvertReturn<()>>::Inner::__ENCODING,
            <bool as ConvertReturn<()>>::Inner::ENCODING
        );
    }

    #[test]
    fn test_impls() {
        let b: Bool = Default::default();
        assert!(!b.as_bool());
        assert!(b.is_false());

        assert!(Bool::from(true).as_bool());
        assert!(Bool::from(true).is_true());
        assert!(Bool::from(false).is_false());

        assert!(Bool::from(true).is_true());
        assert!(Bool::from(false).is_false());

        assert_eq!(Bool::new(true), Bool::new(true));
        assert_eq!(Bool::new(false), Bool::new(false));

        assert!(Bool::new(false) < Bool::new(true));
    }

    #[test]
    fn test_debug() {
        assert_eq!(format!("{:?}", Bool::from(true)), "YES");
        assert_eq!(format!("{:?}", Bool::from(false)), "NO");
    }

    #[test]
    // Test on platform where we know the type of BOOL
    #[cfg(all(target_vendor = "apple", target_os = "macos", target_arch = "x86_64"))]
    fn test_outside_normal() {
        let b = Bool::from_raw(42);
        assert!(b.is_true());
        assert!(!b.is_false());
        assert_eq!(b.as_raw(), 42);

        // PartialEq ignores extra data
        assert_eq!(b, Bool::new(true));
        assert_ne!(b, Bool::new(false));
    }

