
    use super::*;
    use alloc::vec;

    #[test]
    fn parse_container() {
        const KIND: ContainerKind = ContainerKind::Struct;

        #[track_caller]
        fn assert_name(enc: &str, expected: Result<(&str, Vec<EncodingBox>)>) {
            let mut parser = Parser::new(enc);
            assert_eq!(parser.parse_container(KIND), expected);
        }

        assert_name("abc=}", Ok(("abc", vec![])));
        assert_name(
            "abc=ii}",
            Ok(("abc", vec![EncodingBox::Int, EncodingBox::Int])),
        );
        assert_name("_=}.a'", Ok(("_", vec![])));
        assert_name("abc}def", Ok(("abc", vec![])));
        assert_name("=def}", Err(ErrorKind::InvalidIdentifier(KIND)));
        assert_name(".=def}", Err(ErrorKind::InvalidIdentifier(KIND)));
        assert_name("}xyz", Err(ErrorKind::InvalidIdentifier(KIND)));
        assert_name("", Err(ErrorKind::WrongEndContainer(KIND)));
        assert_name("abc", Err(ErrorKind::WrongEndContainer(KIND)));
        assert_name("abc)def", Err(ErrorKind::WrongEndContainer(KIND)));
    }

    #[test]
    fn parse_bitfield() {
        #[track_caller]
        fn assert_bitfield(enc: &str, expected: Result<EncodingBox>) {
            let mut parser = Parser::new(enc);
            assert_eq!(
                parser
                    .parse_encoding_or_none()
                    .and_then(|enc| parser.expect_empty().map(|()| enc)),
                expected
            );
        }

        assert_bitfield("b8", Ok(EncodingBox::BitField(8, None)));
        assert_bitfield("b8C", Err(ErrorKind::NotAllConsumed));
        assert_bitfield(
            "b8C4",
            Ok(EncodingBox::BitField(
                4,
                Some(Box::new((8, EncodingBox::UChar))),
            )),
        );

        assert_bitfield(
            "{s=b8C}",
            Ok(EncodingBox::Struct(
                "s".into(),
                vec![EncodingBox::BitField(8, None), EncodingBox::UChar],
            )),
        );

        assert_bitfield("b2000", Err(ErrorKind::IntegerTooLarge));
        assert_bitfield(
            "b2000c100",
            Ok(EncodingBox::BitField(
                100,
                Some(Box::new((2000, EncodingBox::Char))),
            )),
        );
        assert_bitfield("b2000C257", Err(ErrorKind::IntegerTooLarge));
    }

    #[test]
    fn parse_closing() {
        let mut parser = Parser::new("]");
        assert_eq!(
            parser.parse_encoding_or_none(),
            Err(ErrorKind::Unknown(b']'))
        );
    }

