use super::ToShoutySnakeCase;

macro_rules! t {
    ($t:ident : $s1:expr => $s2:expr) => {
        #[test]
        fn $t() {
            assert_eq!($s1.to_shouty_snake_case(), $s2)
        }
    };
}

t!(test1: "CamelCase" => "CAMEL_CASE");
t!(test2: "This is Human case." => "THIS_IS_HUMAN_CASE");
t!(test3: "MixedUP CamelCase, with some Spaces" => "MIXED_UP_CAMEL_CASE_WITH_SOME_SPACES");
t!(test4: "mixed_up_snake_case with some _spaces" => "MIXED_UP_SNAKE_CASE_WITH_SOME_SPACES");
t!(test5: "kebab-case" => "KEBAB_CASE");
t!(test6: "SHOUTY_SNAKE_CASE" => "SHOUTY_SNAKE_CASE");
t!(test7: "snake_case" => "SNAKE_CASE");
t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "THIS_CONTAINS_ALL_KINDS_OF_WORD_BOUNDARIES");
t!(test9: "XΣXΣ baﬄe" => "XΣXΣ_BAFFLE");
t!(test10: "XMLHttpRequest" => "XML_HTTP_REQUEST");
