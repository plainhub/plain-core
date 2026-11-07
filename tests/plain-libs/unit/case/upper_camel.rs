use super::ToUpperCamelCase;

macro_rules! t {
    ($t:ident : $s1:expr => $s2:expr) => {
        #[test]
        fn $t() {
            assert_eq!($s1.to_upper_camel_case(), $s2)
        }
    };
}

t!(test1: "CamelCase" => "CamelCase");
t!(test2: "This is Human case." => "ThisIsHumanCase");
t!(test3: "MixedUP_CamelCase, with some Spaces" => "MixedUpCamelCaseWithSomeSpaces");
t!(test4: "mixed_up_ snake_case, with some _spaces" => "MixedUpSnakeCaseWithSomeSpaces");
t!(test5: "kebab-case" => "KebabCase");
t!(test6: "SHOUTY_SNAKE_CASE" => "ShoutySnakeCase");
t!(test7: "snake_case" => "SnakeCase");
t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "ThisContainsAllKindsOfWordBoundaries");
t!(test9: "XΣXΣ baﬄe" => "XσxςBaﬄe");
t!(test10: "XMLHttpRequest" => "XmlHttpRequest");
