
    use super::*;

    use crate::{__available_version, available};

    #[test]
    fn test_parse() {
        #[track_caller]
        fn check(expected: (u16, u8, u8), actual: OSVersion) {
            assert_eq!(
                OSVersion {
                    major: expected.0,
                    minor: expected.1,
                    patch: expected.2,
                },
                actual,
            )
        }

        check((1, 0, 0), __available_version!(1));
        check((1, 2, 0), __available_version!(1.2));
        check((1, 2, 3), __available_version!(1.2.3));
        check((9999, 99, 99), __available_version!(9999.99.99));

        // Ensure that the macro handles leading zeroes correctly
        check((10, 0, 0), __available_version!(010));
        check((10, 20, 0), __available_version!(010.020));
        check((10, 20, 30), __available_version!(010.020.030));
        check(
            (10000, 100, 100),
            __available_version!(000010000.00100.00100),
        );
    }

    #[test]
    fn test_compare() {
        #[track_caller]
        fn check_lt(expected: (u16, u8, u8), actual: (u16, u8, u8)) {
            assert!(
                OSVersion {
                    major: expected.0,
                    minor: expected.1,
                    patch: expected.2,
                } < OSVersion {
                    major: actual.0,
                    minor: actual.1,
                    patch: actual.2,
                },
            )
        }

        check_lt((4, 99, 99), (5, 5, 5));
        check_lt((5, 4, 99), (5, 5, 5));
        check_lt((5, 5, 4), (5, 5, 5));

        check_lt((10, 7, 0), (10, 10, 0));
    }

    #[test]
    #[should_panic = "too many parts to version"]
    fn test_too_many_version_parts() {
        let _ = __available_version!(1.2.3 .4);
    }

    #[test]
    #[should_panic = "found invalid digit when parsing version"]
    fn test_macro_with_identifiers() {
        let _ = __available_version!(A.B);
    }

    #[test]
    #[should_panic = "found empty version number part"]
    fn test_empty_version() {
        let _ = __available_version!();
    }

    #[test]
    #[should_panic = "found invalid digit when parsing version"]
    fn test_only_period() {
        let _ = __available_version!(.);
    }

    #[test]
    #[should_panic = "found invalid digit when parsing version"]
    fn test_has_leading_period() {
        let _ = __available_version!(.1);
    }

    #[test]
    #[should_panic = "found empty version number part"]
    fn test_has_trailing_period() {
        let _ = __available_version!(1.);
    }

    #[test]
    #[should_panic = "major version is too large"]
    fn test_major_too_large() {
        let _ = __available_version!(100000);
    }

    #[test]
    #[should_panic = "minor version is too large"]
    fn test_minor_too_large() {
        let _ = __available_version!(1.1000);
    }

    #[test]
    #[should_panic = "patch version is too large"]
    fn test_patch_too_large() {
        let _ = __available_version!(1.1.1000);
    }

    #[test]
    fn test_general_available() {
        // Always available
        assert!(available!(..));

        // Never available
        assert!(!available!());

        // Low versions, always available
        assert!(available!(
            macos = 10.0,
            ios = 1.0,
            tvos = 1.0,
            watchos = 1.0,
            visionos = 1.0,
            ..
        ));

        // High versions, never available
        assert!(!available!(
            macos = 99,
            ios = 99,
            tvos = 99,
            watchos = 99,
            visionos = 99
        ));

        if !cfg!(target_os = "tvos") {
            // Available nowhere except tvOS
            assert!(!available!(tvos = 1.2));

            // Available everywhere, except low tvOS versions
            assert!(available!(tvos = 1.2, ..));
        }
    }

    #[test]
    fn test_u32_roundtrip() {
        let version = OSVersion {
            major: 1000,
            minor: 100,
            patch: 200,
        };
        assert_eq!(version, OSVersion::from_u32(version.to_u32()));
    }

