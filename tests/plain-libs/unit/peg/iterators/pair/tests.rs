use crate::peg::alloc::{borrow::ToOwned, format, string::ToString};
use crate::peg::macros::tests::*;
use crate::peg::parser::Parser;

#[test]
#[cfg(feature = "pretty-print")]
fn test_pretty_print() {
    let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

    let expected = r#"{
  "pos": [
    0,
    3
  ],
  "rule": "a",
  "inner": {
    "pos": [
      1,
      2
    ],
    "pairs": [
      {
        "pos": [
          1,
          2
        ],
        "rule": "b",
        "inner": "b"
      }
    ]
  }
}"#;

    assert_eq!(expected, pair.to_json());
}

#[test]
fn pair_into_inner() {
    let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap(); // the tokens a(b())

    let pairs = pair.into_inner(); // the tokens b()

    assert_eq!(2, pairs.tokens().count());
}

#[test]
fn get_input_of_pair() {
    let input = "abcde";
    let pair = AbcParser::parse(Rule::a, input).unwrap().next().unwrap();

    assert_eq!(input, pair.get_input());
}
#[test]
fn pair_to_string_matches_as_str() {
    let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();

    assert_eq!(pair.to_string(), pair.as_str().to_string());
}
#[test]
fn alternate_format() {
    let pair = AbcParser::parse(Rule::a, "abcde").unwrap().next().unwrap();
    assert_eq!(format!("{pair}"), "abc".to_owned());
    assert_eq!(format!("{pair:#}"), "a(0, 3, [b(1, 2)])".to_owned());
}
