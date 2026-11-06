
    use super::*;

    #[test]
    fn roundtrip() {
        let data = CFData::from_bytes(&[1, 2, 3]);
        assert_eq!(data.to_vec(), [1, 2, 3]);
    }

    #[test]
    fn empty() {
        let data = CFData::from_bytes(&[]);
        assert!(data.is_empty());
        assert_eq!(data.to_vec(), []);
    }

