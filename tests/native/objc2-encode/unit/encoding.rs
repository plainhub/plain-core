
    use super::*;
    use crate::static_str::{static_encoding_str_array, static_encoding_str_len};
    use alloc::boxed::Box;
    use alloc::string::ToString;
    use alloc::vec;
    use core::str::FromStr;

    fn send_sync<T: Send + Sync>() {}

    #[test]
    fn test_send_sync() {
        send_sync::<Encoding>();
    }

    #[test]
    fn smoke() {
        assert!(Encoding::Short.equivalent_to_str("s"));
    }

    #[test]
    fn qualifiers() {
        assert!(Encoding::Void.equivalent_to_str("v"));
        assert!(Encoding::Void.equivalent_to_str("Vv"));
        assert!(Encoding::String.equivalent_to_str("*"));
        assert!(Encoding::String.equivalent_to_str("r*"));
    }

    macro_rules! assert_enc {
        ($(
            fn $name:ident() {
                $encoding:expr;
                $(
                    ~$equivalent_encoding:expr;
                )*
                $(
                    !$not_encoding:expr;
                )*
                $string:literal;
                $(
                    ~$equivalent_string:expr;
                )*
                $(
                    !$not_string:literal;
                )*
            }
        )+) => {$(
            #[test]
            fn $name() {
                const E: Encoding = $encoding;

                // Check PartialEq
                assert_eq!(E, E, "equal");

                // Check Display
                assert_eq!(E.to_string(), $string, "equal to string");

                // Check encoding box
                let boxed = EncodingBox::from_str($string).expect("parse");
                assert_eq!(boxed.to_string(), $string, "parsed");

                // Check equivalence comparisons
                assert!(E.equivalent_to(&E), "equivalent self");
                assert!(E.equivalent_to_str($string), "equivalent self string {}", $string);
                assert!(E.equivalent_to_box(&boxed), "equivalent self boxed");
                $(
                    assert!(E.equivalent_to(&$equivalent_encoding), "equivalent encoding {}", $equivalent_encoding);
                    assert!(E.equivalent_to_str(&$equivalent_encoding.to_string()), "equivalent encoding string");
                    let boxed = EncodingBox::from_str(&$equivalent_encoding.to_string()).expect("parse equivalent encoding");
                    assert!(E.equivalent_to_box(&boxed), "equivalent encoding boxed");
                )*
                $(
                    assert!(E.equivalent_to_str($equivalent_string), "equivalent string {}", $equivalent_string);
                    let boxed = EncodingBox::from_str($equivalent_string).expect("parse equivalent string");
                    assert!(E.equivalent_to_box(&boxed), "equivalent string boxed");
                )*

                // Negative checks
                $(
                    assert_ne!(E, $not_encoding, "not equal");
                    assert!(!E.equivalent_to(&$not_encoding), "not equivalent encoding");
                    assert!(!E.equivalent_to_str(&$not_encoding.to_string()), "not equivalent encoding string");
                    let boxed = EncodingBox::from_str(&$not_encoding.to_string()).expect("parse not equivalent encoding");
                    assert!(!E.equivalent_to_box(&boxed), "not equivalent boxed");
                )*
                $(
                    assert!(!E.equivalent_to_str(&$not_string), "not equivalent string");
                )*

                // Check static str
                const STATIC_ENCODING_DATA: [u8; static_encoding_str_len(&E, NestingLevel::new())] = static_encoding_str_array(&E, NestingLevel::new());
                const STATIC_ENCODING_STR: &str = unsafe { core::str::from_utf8_unchecked(&STATIC_ENCODING_DATA) };
                assert_eq!(STATIC_ENCODING_STR, $string, "static");
            }
        )+};
    }

    assert_enc! {
        fn int() {
            Encoding::Int;
            !Encoding::Char;
            "i";
        }

        fn char() {
            Encoding::Char;
            !Encoding::Int;
            "c";
            // Qualifiers
            ~"rc";
            ~"nc";
            ~"Nc";
            ~"oc";
            ~"Oc";
            ~"Rc";
            ~"Vc";

            !"ri";
        }

        fn block() {
            Encoding::Block;
            ~Encoding::Class;
            ~Encoding::Object;
            !Encoding::Unknown;
            "@?";
        }

        fn object() {
            Encoding::Object;
            ~Encoding::Block;
            ~Encoding::Class;
            !Encoding::Sel;
            "@";
            ~"@\"AnyClassName\"";
            ~"@\"\""; // Empty class name
            ~"@?";
            ~"#";
            !"@\"MyClassName";
            !"@MyClassName\"";
        }

        fn unknown() {
            Encoding::Unknown;
            !Encoding::Block;
            "?";
        }

        fn double() {
            Encoding::Double;
            "d";
        }

        fn bitfield() {
            Encoding::BitField(4, None);
            !Encoding::Int;
            !Encoding::BitField(5, None);
            !Encoding::BitField(4, Some(&(0, Encoding::Bool)));
            "b4";
            !"b4a";
            !"b4c";
            !"b4B";
            !"b";
            !"b-4";
            !"b0B4";
        }

        fn bitfield_gnustep() {
            Encoding::BitField(4, Some(&(16, Encoding::Bool)));
            !Encoding::Int;
            !Encoding::BitField(4, None);
            !Encoding::BitField(5, Some(&(16, Encoding::Bool)));
            !Encoding::BitField(4, Some(&(20, Encoding::Bool)));
            !Encoding::BitField(4, Some(&(16, Encoding::Char)));
            "b16B4";
            !"b4";
            !"b16B";
            !"b20B4";
            !"b16B5";
            !"b16c4";
            !"b4a";
            !"b";
            !"b-4";
        }

        fn atomic() {
            Encoding::Atomic(&Encoding::Int);
            !Encoding::Pointer(&Encoding::Int);
            !Encoding::Atomic(&Encoding::Char);
            !Encoding::Atomic(&Encoding::Atomic(&Encoding::Int));
            "Ai";
        }

        fn atomic_string() {
            Encoding::Atomic(&Encoding::String);
            "A*";
        }

        fn pointer() {
            Encoding::Pointer(&Encoding::Int);
            !Encoding::Atomic(&Encoding::Int);
            !Encoding::Pointer(&Encoding::Char);
            !Encoding::Pointer(&Encoding::Pointer(&Encoding::Int));
            "^i";
        }

        fn array() {
            Encoding::Array(12, &Encoding::Int);
            !Encoding::Int;
            !Encoding::Array(11, &Encoding::Int);
            !Encoding::Array(12, &Encoding::Char);
            "[12i]";
            !"[12i";
        }

        fn struct_() {
            Encoding::Struct("SomeStruct", &[Encoding::Char, Encoding::Int]);
            ~Encoding::Struct("SomeStruct", &[]);
            !Encoding::Union("SomeStruct", &[Encoding::Char, Encoding::Int]);
            !Encoding::Int;
            !Encoding::Struct("SomeStruct", &[Encoding::Int]);
            !Encoding::Struct("SomeStruct", &[Encoding::Char, Encoding::Int, Encoding::Int]);
            !Encoding::Struct("SomeStruct", &[Encoding::Int, Encoding::Char]);
            !Encoding::Struct("AnotherName", &[Encoding::Char, Encoding::Int]);
            "{SomeStruct=ci}";
            ~"{SomeStruct=}";
            !"{SomeStruct}";
            !"{SomeStruct=ic}";
            !"{SomeStruct=malformed";
        }

        fn pointer_struct() {
            Encoding::Pointer(&Encoding::Struct("SomeStruct", &[Encoding::Char, Encoding::Int]));
            ~Encoding::Pointer(&Encoding::Struct("SomeStruct", &[]));
            !Encoding::Pointer(&Encoding::Struct("SomeStruct", &[Encoding::Int, Encoding::Char]));
            !Encoding::Pointer(&Encoding::Struct("AnotherName", &[Encoding::Char, Encoding::Int]));
            "^{SomeStruct=ci}";
            ~"^{SomeStruct=}";
            !"^{SomeStruct}";
            !"^{SomeStruct=ic}";
            !"^{SomeStruct=malformed";
        }

        fn pointer_pointer_struct() {
            Encoding::Pointer(&Encoding::Pointer(&Encoding::Struct("SomeStruct", &[Encoding::Char, Encoding::Int])));
            ~Encoding::Pointer(&Encoding::Pointer(&Encoding::Struct("SomeStruct", &[])));
            ~Encoding::Pointer(&Encoding::Pointer(&Encoding::Struct("SomeStruct", &[Encoding::Int, Encoding::Char])));
            !Encoding::Pointer(&Encoding::Pointer(&Encoding::Struct("AnotherName", &[Encoding::Char, Encoding::Int])));
            "^^{SomeStruct}";
            !"^^{SomeStruct=}";
            !"^^{SomeStruct=ci}";
            !"^^{SomeStruct=ic}";
            !"^^{AnotherName=ic}";
            !"^^{SomeStruct=malformed";
        }

        fn atomic_struct() {
            Encoding::Atomic(&Encoding::Struct("SomeStruct", &[Encoding::Char, Encoding::Int]));
            ~Encoding::Atomic(&Encoding::Struct("SomeStruct", &[]));
            ~Encoding::Atomic(&Encoding::Struct("SomeStruct", &[Encoding::Int, Encoding::Char]));
            !Encoding::Atomic(&Encoding::Struct("AnotherName", &[Encoding::Char, Encoding::Int]));
            "A{SomeStruct}";
            !"A{SomeStruct=}";
            !"A{SomeStruct=ci}";
            !"A{SomeStruct=ic}";
            !"A{SomeStruct=malformed";
        }

        fn empty_struct() {
            Encoding::Struct("SomeStruct", &[]);
            "{SomeStruct=}";
            ~"{SomeStruct=ci}";
            !"{SomeStruct}";
        }

        fn union_() {
            Encoding::Union("Onion", &[Encoding::Char, Encoding::Int]);
            !Encoding::Struct("Onion", &[Encoding::Char, Encoding::Int]);
            !Encoding::Int;
            !Encoding::Union("Onion", &[Encoding::Int, Encoding::Char]);
            !Encoding::Union("AnotherUnion", &[Encoding::Char, Encoding::Int]);
            "(Onion=ci)";
            !"(Onion=ci";
        }

        fn nested() {
            Encoding::Struct(
                "A",
                &[
                    Encoding::Struct("B", &[Encoding::Int]),
                    Encoding::Pointer(&Encoding::Struct("C", &[Encoding::Double])),
                    Encoding::Char,
                ],
            );
            ~Encoding::Struct(
                "A",
                &[
                    Encoding::Struct("B", &[Encoding::Int]),
                    Encoding::Pointer(&Encoding::Struct("C", &[])),
                    Encoding::Char,
                ],
            );
            "{A={B=i}^{C}c}";
            !"{A={B=i}^{C=d}c}";
            !"{A={B=i}^{C=i}c}";
            !"{A={B=i}^{C=d}c";
        }

        fn nested_pointer() {
            Encoding::Pointer(&Encoding::Struct(
                "A",
                &[
                    Encoding::Struct("B", &[Encoding::Int]),
                    Encoding::Pointer(&Encoding::Struct("C", &[Encoding::Double])),
                ],
            ));
            "^{A={B=i}^{C}}";
            !"^{A={B}^{C}}";
            !"^{A={B=i}^{C=d}}";
        }

        fn various() {
            Encoding::Struct(
                "abc",
                &[
                    Encoding::Pointer(&Encoding::Array(8, &Encoding::Bool)),
                    Encoding::Union("def", &[Encoding::Block]),
                    Encoding::Pointer(&Encoding::Pointer(&Encoding::BitField(255, None))),
                    Encoding::Char,
                    Encoding::Unknown,
                ]
            );
            "{abc=^[8B](def=@?)^^b255c?}";
            ~"{abc=}";
            !"{abc}";
        }

        fn identifier() {
            Encoding::Struct("_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789", &[]);
            "{_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789=}";
        }

        // Regression test. The encoding of the `CGLContextObj` object changed
        // between versions of macOS. As such, this is something that we must
        // be prepared to handle.
        fn cgl_context_obj() {
            Encoding::Pointer(&Encoding::Struct("_CGLContextObject", &[]));
            "^{_CGLContextObject=}";
            ~"^{_CGLContextObject=^{__GLIContextRec}{__GLIFunctionDispatchRec=^?^?^?^?^?}^{_CGLPrivateObject}^v}";
            !"^{_CGLContextObject}";
            !"^{SomeOtherStruct=}";
        }

        fn none() {
            Encoding::None;
            "";
            !"?";
        }

        fn none_in_array() {
            Encoding::Array(42, &Encoding::None);
            !Encoding::Array(42, &Encoding::Unknown);
            "[42]";
            !"[42i]";
        }

        fn none_in_pointer() {
            Encoding::Pointer(&Encoding::None);
            !Encoding::Pointer(&Encoding::Unknown);
            "^";
            !"";
            !"^i";
        }

        fn none_in_pointer_in_array() {
            Encoding::Array(42, &Encoding::Pointer(&Encoding::None));
            "[42^]";
        }

        fn class() {
            Encoding::Class;
            ~Encoding::Object;
            ~Encoding::Block;
            !Encoding::Sel;
            "#";
            ~"@?";
            ~"@";
            !"a";
        }
    }

    #[test]
    #[should_panic = "Struct name was not a valid identifier"]
    fn struct_empty() {
        let _ = Encoding::Struct("", &[]).to_string();
    }

    #[test]
    #[should_panic = "Struct name was not a valid identifier"]
    fn struct_unicode() {
        let _ = Encoding::Struct("☃", &[Encoding::Char]).to_string();
    }

    #[test]
    #[should_panic = "Union name was not a valid identifier"]
    fn union_invalid_identifier() {
        let _ = Encoding::Union("a-b", &[Encoding::Char]).equivalent_to_str("(☃=c)");
    }

    // Note: A raw `?` cannot happen in practice, since functions can only
    // be accessed through pointers, and that will yield `^?`
    #[test]
    fn object_unknown_in_struct() {
        let enc = Encoding::Struct("S", &[Encoding::Block, Encoding::Object, Encoding::Unknown]);
        let s = "{S=@?@?}";

        assert_eq!(&enc.to_string(), s);

        let parsed = EncodingBox::from_str(s).unwrap();
        let expected = EncodingBox::Struct(
            "S".to_string(),
            vec![EncodingBox::Block, EncodingBox::Block],
        );
        assert_eq!(parsed, expected);

        assert!(!enc.equivalent_to_box(&expected));
    }

    // Similar to `?`, `` cannot be accurately represented inside pointers
    // inside structs, and may be parsed incorrectly.
    #[test]
    fn none_in_struct() {
        let enc = Encoding::Struct("?", &[Encoding::Pointer(&Encoding::None), Encoding::Int]);
        let s = "{?=^i}";
        assert_eq!(&enc.to_string(), s);

        let parsed = EncodingBox::from_str(s).unwrap();
        let expected = EncodingBox::Struct(
            "?".to_string(),
            vec![EncodingBox::Pointer(Box::new(EncodingBox::Int))],
        );
        assert_eq!(parsed, expected);

        assert!(!enc.equivalent_to_box(&expected));
    }

