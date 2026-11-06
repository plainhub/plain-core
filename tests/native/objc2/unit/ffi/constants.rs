
    use super::*;
    #[test]
    fn test_association_policy() {
        assert_eq!(OBJC_ASSOCIATION_RETAIN, 769);
        assert_eq!(OBJC_ASSOCIATION_COPY, 771);

        // What the GNUStep headers define
        assert_eq!(OBJC_ASSOCIATION_RETAIN, 0x301);
        assert_eq!(OBJC_ASSOCIATION_COPY, 0x303);
    }

