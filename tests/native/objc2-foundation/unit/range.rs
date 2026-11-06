
    use super::*;

    #[test]
    fn test_from_range() {
        let cases: &[(Range<usize>, NSRange)] = &[
            (0..0, NSRange::new(0, 0)),
            (0..10, NSRange::new(0, 10)),
            (10..10, NSRange::new(10, 0)),
            (10..20, NSRange::new(10, 10)),
        ];

        for (range, expected) in cases {
            assert_eq!(NSRange::from(range.clone()), *expected);
        }
    }

    #[test]
    #[should_panic = "Range end < start"]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_from_range_inverted() {
        let _ = NSRange::from(10..0);
    }

    #[test]
    fn test_contains() {
        let range = NSRange::from(10..20);
        assert!(!range.contains(0));
        assert!(!range.contains(9));
        assert!(range.contains(10));
        assert!(range.contains(11));
        assert!(!range.contains(20));
        assert!(!range.contains(21));
    }

    #[test]
    fn test_end() {
        let range = NSRange::from(10..20);
        assert!(!range.contains(0));
        assert!(!range.contains(9));
        assert!(range.contains(10));
        assert!(range.contains(11));
        assert!(!range.contains(20));
        assert!(!range.contains(21));
    }

    #[test]
    #[should_panic = "NSRange too large"]
    fn test_end_large() {
        let _ = NSRange::new(usize::MAX, usize::MAX).end();
    }

