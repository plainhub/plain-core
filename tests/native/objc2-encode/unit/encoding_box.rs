
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    #[test]
    fn eq_encodings() {
        let enc1 = Encoding::Char;
        let enc2 = EncodingBox::Char;
        let enc3 = EncodingBox::String;
        assert_eq!(enc1, enc2);
        assert_ne!(enc1, enc3);
    }

    #[test]
    fn eq_complex_encodings() {
        let enc1 = Encoding::Atomic(&Encoding::Struct(
            "test",
            &[Encoding::Array(2, &Encoding::Int)],
        ));
        let enc2 = EncodingBox::Atomic(Box::new(EncodingBox::Struct(
            "test".to_string(),
            vec![EncodingBox::Array(2, Box::new(EncodingBox::Int))],
        )));
        let enc3 = EncodingBox::Atomic(Box::new(EncodingBox::Struct(
            "test".to_string(),
            vec![EncodingBox::Array(2, Box::new(EncodingBox::Char))],
        )));
        assert_eq!(enc1, enc2);
        assert_ne!(enc1, enc3);
    }

    #[test]
    fn struct_nested_in_pointer() {
        let enc1 = EncodingBox::Struct("test".to_string(), vec![EncodingBox::Char]);
        let enc2 = EncodingBox::Struct("test".to_string(), vec![EncodingBox::Int]);
        const ENC3A: Encoding = Encoding::Struct("test", &[Encoding::Char]);
        assert_ne!(enc1, enc2);
        assert!(ENC3A.equivalent_to_box(&enc1));
        assert!(!ENC3A.equivalent_to_box(&enc2));

        let enc1 = EncodingBox::Pointer(Box::new(enc1));
        let enc2 = EncodingBox::Pointer(Box::new(enc2));
        const ENC3B: Encoding = Encoding::Pointer(&ENC3A);
        assert_ne!(enc1, enc2);
        assert!(ENC3B.equivalent_to_box(&enc1));
        assert!(!ENC3B.equivalent_to_box(&enc2));

        let enc1 = EncodingBox::Pointer(Box::new(enc1));
        let enc2 = EncodingBox::Pointer(Box::new(enc2));
        const ENC3C: Encoding = Encoding::Pointer(&ENC3B);
        assert_ne!(enc1, enc2);
        assert!(ENC3C.equivalent_to_box(&enc1));
        assert!(ENC3C.equivalent_to_box(&enc2), "now they're equivalent");
    }

    #[test]
    fn parse_atomic_struct() {
        let expected = EncodingBox::Atomic(Box::new(EncodingBox::Atomic(Box::new(
            EncodingBox::Struct("a".into(), vec![]),
        ))));
        let actual = EncodingBox::from_str("AA{a=}").unwrap();
        assert_eq!(expected, actual);
        assert_eq!(expected.to_string(), "AA{a}");

        let actual = EncodingBox::from_str("AA{a}").unwrap();
        assert_eq!(expected, actual);
        assert_eq!(expected.to_string(), "AA{a}");
    }

    #[test]
    fn parse_part_of_string() {
        let mut s = "{a}cb0i16";

        let expected = EncodingBox::Struct("a".into(), vec![]);
        let actual = EncodingBox::from_start_of_str(&mut s).unwrap();
        assert_eq!(expected, actual);

        let expected = EncodingBox::Char;
        let actual = EncodingBox::from_start_of_str(&mut s).unwrap();
        assert_eq!(expected, actual);

        let expected = EncodingBox::BitField(16, Some(Box::new((0, EncodingBox::Int))));
        let actual = EncodingBox::from_start_of_str(&mut s).unwrap();
        assert_eq!(expected, actual);

        assert_eq!(s, "");
    }

