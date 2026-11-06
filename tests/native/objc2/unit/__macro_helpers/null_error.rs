
    use super::*;

    #[test]
    fn test_is_eq() {
        assert!(is_eq("NSError", "NSError"));
        assert!(!is_eq("nserror", "NSError"));
        assert!(!is_eq("CFError", "NSError"));
        assert!(!is_eq("NSErr", "NSError"));
        assert!(!is_eq("NSErrorrrr", "NSError"));
    }

    #[test]
    fn test_create() {
        let _ = create_null_error().0;
    }

