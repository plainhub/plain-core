
    use super::*;
    use alloc::boxed::Box;
    use alloc::vec;
    use alloc::vec::Vec;

    fn assert_stack_layout(mut types: &str, expected: Option<isize>, rest: &str) {
        let sl = parse_stack_layout(&mut types).unwrap();
        assert_eq!(sl, expected);
        assert_eq!(types, rest);
    }

    #[test]
    fn stack_layout_extract() {
        assert_stack_layout("", None, "");
        assert_stack_layout("abc", None, "abc");
        assert_stack_layout("abc12abc", None, "abc12abc");
        assert_stack_layout("0", Some(0), "");
        assert_stack_layout("1abc", Some(1), "abc");
        assert_stack_layout("42def24", Some(42), "def24");
        assert_stack_layout("1234567890xyz", Some(1234567890), "xyz");

        assert_stack_layout("-1a", Some(-1), "a");
        assert_stack_layout("-1a", Some(-1), "a");

        // GNU runtime's register parameter hint??
        assert_stack_layout("+1a", Some(1), "a");
    }

    fn assert_encoding_extract(s: &str, expected: &[(EncodingBox, Option<isize>)]) {
        let actual: Vec<_> = MethodEncodingIter::new(s)
            .collect::<Result<_, _>>()
            .unwrap_or_else(|e| panic!("{}", e));
        assert_eq!(&actual, expected);
    }

    #[test]
    fn parse_bitfield() {
        assert_encoding_extract(
            "@48@0:8Ad16r^*24{bitfield=b64b1}32i48",
            &[
                (EncodingBox::Object, Some(48)),
                (EncodingBox::Object, Some(0)),
                (EncodingBox::Sel, Some(8)),
                (EncodingBox::Atomic(Box::new(EncodingBox::Double)), Some(16)),
                (
                    EncodingBox::Pointer(Box::new(EncodingBox::String)),
                    Some(24),
                ),
                (
                    EncodingBox::Struct(
                        "bitfield".into(),
                        vec![
                            EncodingBox::BitField(64, None),
                            EncodingBox::BitField(1, None),
                        ],
                    ),
                    Some(32),
                ),
                (EncodingBox::Int, Some(48)),
            ],
        );
    }

    #[test]
    fn parse_complex() {
        assert_encoding_extract(
            "jf16@0:8",
            &[
                (EncodingBox::FloatComplex, Some(16)),
                (EncodingBox::Object, Some(0)),
                (EncodingBox::Sel, Some(8)),
            ],
        );
        assert_encoding_extract(
            "jf@:",
            &[
                (EncodingBox::FloatComplex, None),
                (EncodingBox::Object, None),
                (EncodingBox::Sel, None),
            ],
        );
    }

