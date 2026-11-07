use super::super::super::Parser;
use super::super::super::macros::tests::*;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

#[test]
#[cfg(feature = "pretty-print")]
fn test_pretty_print() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    let expected = r#"{
  "pos": [
    0,
    5
  ],
  "pairs": [
    {
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
    },
    {
      "pos": [
        4,
        5
      ],
      "rule": "c",
      "inner": "e"
    }
  ]
}"#;

    assert_eq!(expected, pairs.to_json());
}

#[test]
fn as_str() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    assert_eq!(pairs.as_str(), "abcde");
}

#[test]
fn get_input_of_pairs() {
    let input = "abcde";
    let pairs = AbcParser::parse(Rule::a, input).unwrap();

    assert_eq!(pairs.get_input(), input);
}

#[test]
fn as_str_empty() {
    let mut pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    assert_eq!(pairs.nth(1).unwrap().into_inner().as_str(), "");
}

#[test]
fn concat() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    assert_eq!(pairs.concat(), "abce");
}

#[test]
fn pairs_debug() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    #[rustfmt::skip]
        assert_eq!(
            format!("{pairs:?}"),
            "[\
                Pair { rule: a, span: Span { str: \"abc\", range: 0..3 }, inner: [\
                    Pair { rule: b, span: Span { str: \"b\", range: 1..2 }, inner: [] }\
                ] }, \
                Pair { rule: c, span: Span { str: \"e\", range: 4..5 }, inner: [] }\
            ]"
            .to_owned()
        );
}

#[test]
fn pairs_display() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    assert_eq!(format!("{pairs}"), "[abc, e]".to_owned());
    assert_eq!(
        format!("{pairs:#}"),
        "[a(0, 3, [b(1, 2)]), c(4, 5)]".to_owned()
    );
}

#[test]
fn iter_for_pairs() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();
    assert_eq!(
        pairs.map(|p| p.as_rule()).collect::<Vec<Rule>>(),
        vec![Rule::a, Rule::c]
    );
}

#[test]
fn double_ended_iter_for_pairs() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();
    assert_eq!(
        pairs.rev().map(|p| p.as_rule()).collect::<Vec<Rule>>(),
        vec![Rule::c, Rule::a]
    );
}

#[test]
fn test_line_col() {
    let mut pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap();
    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "abc");
    assert_eq!(pair.line_col(), (1, 1));

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "e");
    assert_eq!(pair.line_col(), (2, 1));

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "fgh");
    assert_eq!(pair.line_col(), (2, 2));
}

#[test]
fn test_rev_iter_line_col() {
    let mut pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap().rev();
    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "fgh");
    assert_eq!(pair.line_col(), (2, 2));

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "e");
    assert_eq!(pair.line_col(), (2, 1));

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "abc");
    assert_eq!(pair.line_col(), (1, 1));
}

#[test]
// false positive: pest uses `..` as a complete range (historically)

fn test_tag_node_branch() {
    use crate::peg::{ParseResult, ParserState, state};

    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    enum Rule {
        number, // 0..9
        add,    // num + num
        mul,    // num * num
    }
    fn mark_branch(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        expr(state, Rule::mul, "*")
            .and_then(|state| state.tag_node("mul"))
            .or_else(|state| expr(state, Rule::add, "+"))
            .and_then(|state| state.tag_node("add"))
    }
    fn expr<'a>(
        state: Box<ParserState<'a, Rule>>,
        r: Rule,
        o: &'static str,
    ) -> ParseResult<Box<ParserState<'a, Rule>>> {
        state.rule(r, |state| {
            state.sequence(|state| {
                number(state)
                    .and_then(|state| state.tag_node("lhs"))
                    .and_then(|state| state.match_string(o))
                    .and_then(number)
                    .and_then(|state| state.tag_node("rhs"))
            })
        })
    }
    fn number(state: Box<ParserState<'_, Rule>>) -> ParseResult<Box<ParserState<'_, Rule>>> {
        state.rule(Rule::number, |state| state.match_range('0'..'9'))
    }
    let input = "1+2";
    let pairs = state(input, mark_branch).unwrap();
    assert_eq!(pairs.find_first_tagged("add").unwrap().as_rule(), Rule::add);
    assert_eq!(pairs.find_first_tagged("mul"), None);

    let mut left_numbers = pairs.clone().find_tagged("lhs");

    assert_eq!(left_numbers.next().unwrap().as_str(), "1");
    assert_eq!(left_numbers.next(), None);
    let mut right_numbers = pairs.find_tagged("rhs");

    assert_eq!(right_numbers.next().unwrap().as_str(), "2");
    assert_eq!(right_numbers.next(), None);
}

#[test]
fn exact_size_iter_for_pairs() {
    let pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap();
    assert_eq!(pairs.len(), pairs.count());

    let pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap().rev();
    assert_eq!(pairs.len(), pairs.count());

    let mut pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap();
    let pairs_len = pairs.len();
    let _ = pairs.next().unwrap();
    assert_eq!(pairs.count() + 1, pairs_len);
}
