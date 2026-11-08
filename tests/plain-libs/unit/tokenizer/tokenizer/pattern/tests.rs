use super::*;
use crate::regex;

macro_rules! do_test {
    ($inside: expr, $pattern: expr => @ERROR) => {
        assert!($pattern.find_matches($inside).is_err());
    };
    ($inside: expr, $pattern: expr => $result: expr) => {
        assert_eq!($pattern.find_matches($inside).unwrap(), $result);
        assert_eq!(
            Invert($pattern).find_matches($inside).unwrap(),
            $result
                .into_iter()
                .map(|v: (Offsets, bool)| (v.0, !v.1))
                .collect::<Vec<_>>()
        );
    };
}

#[test]
fn char() {
    do_test!("aba", 'a' => vec![((0, 1), true), ((1, 2), false), ((2, 3), true)]);
    do_test!("bbbba", 'a' => vec![((0, 4), false), ((4, 5), true)]);
    do_test!("aabbb", 'a' => vec![((0, 1), true), ((1, 2), true), ((2, 5), false)]);
    do_test!("", 'a' => vec![((0, 0), false)]);
    do_test!("aaa", 'b' => vec![((0, 3), false)]);
}

#[test]
fn str() {
    do_test!("aba", "a" => vec![((0, 1), true), ((1, 2), false), ((2, 3), true)]);
    do_test!("bbbba", "a" => vec![((0, 4), false), ((4, 5), true)]);
    do_test!("aabbb", "a" => vec![((0, 1), true), ((1, 2), true), ((2, 5), false)]);
    do_test!("aabbb", "ab" => vec![((0, 1), false), ((1, 3), true), ((3, 5), false)]);
    do_test!("aabbab", "ab" =>
        vec![((0, 1), false), ((1, 3), true), ((3, 4), false), ((4, 6), true)]
    );
    do_test!("", "" => vec![((0, 0), false)]);
    do_test!("aaa", "" => vec![((0, 3), false)]);
    do_test!("aaa", "b" => vec![((0, 3), false)]);
}

#[test]
fn functions() {
    let is_b = |c| c == 'b';
    do_test!("aba", is_b => vec![((0, 1), false), ((1, 2), true), ((2, 3), false)]);
    do_test!("aaaab", is_b => vec![((0, 4), false), ((4, 5), true)]);
    do_test!("bbaaa", is_b => vec![((0, 1), true), ((1, 2), true), ((2, 5), false)]);
    do_test!("", is_b => vec![((0, 0), false)]);
    do_test!("aaa", is_b => vec![((0, 3), false)]);
}

#[test]
fn regex() {
    let is_whitespace = regex::compile(r"\s+").unwrap();
    do_test!("a   b", &is_whitespace => vec![((0, 1), false), ((1, 4), true), ((4, 5), false)]);
    do_test!("   a   b   ", &is_whitespace =>
        vec![((0, 3), true), ((3, 4), false), ((4, 7), true), ((7, 8), false), ((8, 11), true)]
    );
    do_test!("", &is_whitespace => vec![((0, 0), false)]);
    do_test!("𝔾𝕠𝕠𝕕 𝕞𝕠𝕣𝕟𝕚𝕟𝕘", &is_whitespace =>
        vec![((0, 16), false), ((16, 17), true), ((17, 45), false)]
    );
    do_test!("aaa", &is_whitespace => vec![((0, 3), false)]);
}

#[test]
fn sys_regex() {
    let is_whitespace = SysRegex::new(r"\s+").unwrap();
    do_test!("a   b", &is_whitespace => vec![((0, 1), false), ((1, 4), true), ((4, 5), false)]);
    do_test!("   a   b   ", &is_whitespace =>
        vec![((0, 3), true), ((3, 4), false), ((4, 7), true), ((7, 8), false), ((8, 11), true)]
    );
    do_test!("", &is_whitespace => vec![((0, 0), false)]);
    do_test!("𝔾𝕠𝕠𝕕 𝕞𝕠𝕣𝕟𝕚𝕟𝕘", &is_whitespace =>
        vec![((0, 16), false), ((16, 17), true), ((17, 45), false)]
    );
    do_test!("aaa", &is_whitespace => vec![((0, 3), false)]);
}
