use super::*;

    #[test]
    fn test_convert_case() {
        let id = Ident::new("test_me", proc_macro2::Span::call_site());
        assert_eq!("testMe", id.convert_case(Some(CaseStyle::CamelCase)));
        assert_eq!("TestMe", id.convert_case(Some(CaseStyle::PascalCase)));
        assert_eq!("Test-Me", id.convert_case(Some(CaseStyle::TrainCase)));
    }

    #[test]
    fn test_impl_from_str_for_case_style_pascal_case() {
        use CaseStyle::*;
        let f = CaseStyle::from_str;

        assert_eq!(PascalCase, f("PascalCase").unwrap());
        assert_eq!(PascalCase, f("camel_case").unwrap());

        assert_eq!(CamelCase, f("camelCase").unwrap());

        assert_eq!(SnakeCase, f("snake_case").unwrap());
        assert_eq!(SnakeCase, f("snek_case").unwrap());

        assert_eq!(KebabCase, f("kebab-case").unwrap());
        assert_eq!(KebabCase, f("kebab_case").unwrap());

        assert_eq!(ScreamingKebabCase, f("SCREAMING-KEBAB-CASE").unwrap());

        assert_eq!(ShoutySnakeCase, f("SCREAMING_SNAKE_CASE").unwrap());
        assert_eq!(ShoutySnakeCase, f("shouty_snake_case").unwrap());
        assert_eq!(ShoutySnakeCase, f("shouty_snek_case").unwrap());

        assert_eq!(LowerCase, f("lowercase").unwrap());

        assert_eq!(UpperCase, f("UPPERCASE").unwrap());

        assert_eq!(TitleCase, f("title_case").unwrap());

        assert_eq!(MixedCase, f("mixed_case").unwrap());
    }
