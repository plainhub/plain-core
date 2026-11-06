
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn test_in_selector_family() {
        #[track_caller]
        fn assert_in_family(selector: &str, family: &str) {
            assert!(in_selector_family(selector.as_bytes(), family.as_bytes()));
            let selector = selector.to_string() + "\0";
            assert!(in_selector_family(selector.as_bytes(), family.as_bytes()));
        }

        #[track_caller]
        fn assert_not_in_family(selector: &str, family: &str) {
            assert!(!in_selector_family(selector.as_bytes(), family.as_bytes()));
            let selector = selector.to_string() + "\0";
            assert!(!in_selector_family(selector.as_bytes(), family.as_bytes()));
        }

        // Common cases

        assert_in_family("alloc", "alloc");
        assert_in_family("allocWithZone:", "alloc");
        assert_not_in_family("dealloc", "alloc");
        assert_not_in_family("initialize", "init");
        assert_not_in_family("decimalNumberWithDecimal:", "init");
        assert_in_family("initWithCapacity:", "init");
        assert_in_family("_initButPrivate:withParam:", "init");
        assert_not_in_family("description", "init");
        assert_not_in_family("inIT", "init");

        assert_not_in_family("init", "copy");
        assert_not_in_family("copyingStuff:", "copy");
        assert_in_family("copyWithZone:", "copy");
        assert_not_in_family("initWithArray:copyItems:", "copy");
        assert_in_family("copyItemAtURL:toURL:error:", "copy");

        assert_not_in_family("mutableCopying", "mutableCopy");
        assert_in_family("mutableCopyWithZone:", "mutableCopy");
        assert_in_family("mutableCopyWithZone:", "mutableCopy");

        assert_in_family(
            "newScriptingObjectOfClass:forValueForKey:withContentsValue:properties:",
            "new",
        );
        assert_in_family(
            "newScriptingObjectOfClass:forValueForKey:withContentsValue:properties:",
            "new",
        );
        assert_not_in_family("newsstandAssetDownload", "new");

        // Trying to weed out edge-cases:

        assert_in_family("__abcDef", "abc");
        assert_in_family("_abcDef", "abc");
        assert_in_family("abcDef", "abc");
        assert_in_family("___a", "a");
        assert_in_family("__a", "a");
        assert_in_family("_a", "a");
        assert_in_family("a", "a");

        assert_not_in_family("_abcdef", "abc");
        assert_not_in_family("_abcdef", "def");
        assert_not_in_family("_bcdef", "abc");
        assert_not_in_family("a_bc", "abc");
        assert_not_in_family("abcdef", "abc");
        assert_not_in_family("abcdef", "def");
        assert_not_in_family("abcdef", "abb");
        assert_not_in_family("___", "a");
        assert_not_in_family("_", "a");
        assert_not_in_family("", "a");

        assert_in_family("copy", "copy");
        assert_in_family("copy:", "copy");
        assert_in_family("copyMe", "copy");
        assert_in_family("_copy", "copy");
        assert_in_family("_copy:", "copy");
        assert_in_family("_copyMe", "copy");
        assert_not_in_family("copying", "copy");
        assert_not_in_family("copying:", "copy");
        assert_not_in_family("_copying", "copy");
        assert_not_in_family("Copy", "copy");
        assert_not_in_family("COPY", "copy");

        // Empty family (not supported)
        assert_in_family("___", "");
        assert_in_family("__", "");
        assert_in_family("_", "");
        assert_in_family("", "");
        assert_not_in_family("_a", "");
        assert_not_in_family("a", "");
        assert_in_family("_A", "");
        assert_in_family("A", "");

        // Double-colon selectors
        assert_in_family("abc::abc::", "abc");
        assert_in_family("abc:::", "abc");
        assert_in_family("abcDef::xyz:", "abc");
        // Invalid selector (probably)
        assert_not_in_family("::abc:", "abc");
    }

    #[test]
    fn test_method_family() {
        #[track_caller]
        fn assert_types_eq<T: 'static, U: 'static>() {
            assert_eq!(std::any::TypeId::of::<T>(), std::any::TypeId::of::<U>());
        }

        assert_types_eq::<AllocFamily, __method_family!(()(alloc))>();
        assert_types_eq::<AllocFamily, __method_family!(()(allocWithZone:))>();
        assert_types_eq::<CopyFamily, __method_family!(()(copyItemAtURL:toURL:error:))>();
        assert_types_eq::<NewFamily, __method_family!(()(new))>();
        assert_types_eq::<InitFamily, __method_family!(()(initWithArray:))>();
        assert_types_eq::<NoneFamily, __method_family!(()(somethingElse:))>();

        assert_types_eq::<CopyFamily, __method_family!((copy)(initWithArray:))>();
    }

