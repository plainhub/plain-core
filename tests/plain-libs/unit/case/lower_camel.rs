use super::ToLowerCamelCase;

macro_rules! t {
    ($t:ident : $s1:expr => $s2:expr) => {
        #[test]
        fn $t() {
            assert_eq!($s1.to_lower_camel_case(), $s2)
        }
    };
}

t!(test1: "CamelCase" => "camelCase");
t!(test2: "This is Human case." => "thisIsHumanCase");
t!(test3: "MixedUP CamelCase, with some Spaces" => "mixedUpCamelCaseWithSomeSpaces");
t!(test4: "mixed_up_ snake_case, with some _spaces" => "mixedUpSnakeCaseWithSomeSpaces");
t!(test5: "kebab-case" => "kebabCase");
t!(test6: "SHOUTY_SNAKE_CASE" => "shoutySnakeCase");
t!(test7: "snake_case" => "snakeCase");
t!(test8: "this-contains_ ALLKinds OfWord_Boundaries" => "thisContainsAllKindsOfWordBoundaries");
t!(test9: "XΣXΣ baﬄe" => "xσxςBaﬄe");
t!(test10: "XMLHttpRequest" => "xmlHttpRequest");
