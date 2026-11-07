use super::*;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Rule {
    a,
    b,
    sum,
    number,
}

#[test]
fn single_leaf() {
    let pairs = PairsBuilder::new("42").rule(Rule::number, 0, 2).build();
    let pair = pairs.peek().unwrap();

    assert_eq!(pair.as_rule(), Rule::number);
    assert_eq!(pair.as_str(), "42");
    assert_eq!(pair.as_span().start(), 0);
    assert_eq!(pair.as_span().end(), 2);
    assert_eq!(pair.into_inner().count(), 0);
}

#[test]
fn multiple_top_level_leaves() {
    let mut pairs = PairsBuilder::new("a b")
        .rule(Rule::a, 0, 1)
        .rule(Rule::b, 2, 3)
        .build();

    assert_eq!(pairs.len(), 2);

    let first = pairs.next().unwrap();
    assert_eq!(first.as_rule(), Rule::a);
    assert_eq!(first.as_str(), "a");

    let second = pairs.next().unwrap();
    assert_eq!(second.as_rule(), Rule::b);
    assert_eq!(second.as_str(), "b");

    assert!(pairs.next().is_none());
}

#[test]
fn nested_pairs() {
    let pairs = PairsBuilder::new("1+2")
        .rule_with(Rule::sum, 0, 3, |inner| {
            inner.rule(Rule::number, 0, 1).rule(Rule::number, 2, 3)
        })
        .build();

    let sum = pairs.peek().unwrap();
    assert_eq!(sum.as_rule(), Rule::sum);
    assert_eq!(sum.as_str(), "1+2");

    let inner: Vec<_> = sum.into_inner().collect();
    assert_eq!(inner.len(), 2);
    assert_eq!(inner[0].as_str(), "1");
    assert_eq!(inner[0].as_rule(), Rule::number);
    assert_eq!(inner[1].as_str(), "2");
    assert_eq!(inner[1].as_rule(), Rule::number);
}

#[test]
fn tokens_round_trip() {
    use crate::peg::Token;

    let pairs = PairsBuilder::new("1+2")
        .rule_with(Rule::sum, 0, 3, |inner| {
            inner.rule(Rule::number, 0, 1).rule(Rule::number, 2, 3)
        })
        .build();

    let tokens: Vec<_> = pairs.tokens().collect();
    assert_eq!(
        tokens,
        vec![
            Token::Start {
                rule: Rule::sum,
                pos: pos("1+2", 0)
            },
            Token::Start {
                rule: Rule::number,
                pos: pos("1+2", 0)
            },
            Token::End {
                rule: Rule::number,
                pos: pos("1+2", 1)
            },
            Token::Start {
                rule: Rule::number,
                pos: pos("1+2", 2)
            },
            Token::End {
                rule: Rule::number,
                pos: pos("1+2", 3)
            },
            Token::End {
                rule: Rule::sum,
                pos: pos("1+2", 3)
            },
        ]
    );
}

fn pos(input: &str, pos: usize) -> crate::peg::Position<'_> {
    crate::peg::Position::new(input, pos).unwrap()
}

#[test]
fn tags_are_queryable() {
    let pairs = PairsBuilder::new("1+2")
        .rule_with(Rule::sum, 0, 3, |inner| {
            inner
                .rule(Rule::number, 0, 1)
                .tag("lhs")
                .rule(Rule::number, 2, 3)
                .tag("rhs")
        })
        .build();

    assert_eq!(
        pairs.clone().find_first_tagged("lhs").unwrap().as_str(),
        "1"
    );
    assert_eq!(
        pairs.clone().find_first_tagged("rhs").unwrap().as_str(),
        "2"
    );
    assert!(pairs.find_first_tagged("missing").is_none());
}

#[test]
fn empty_builder_yields_no_pairs() {
    let pairs = PairsBuilder::<Rule>::new("").build();
    assert!(pairs.clone().peek().is_none());
    assert_eq!(pairs.count(), 0);
}

#[test]
fn line_col_is_computed() {
    let pairs = PairsBuilder::new("ab\ncd").rule(Rule::a, 3, 5).build();
    let pair = pairs.peek().unwrap();
    assert_eq!(pair.line_col(), (2, 1));
}

#[test]
fn multibyte_span() {
    // "héllo": 'é' is two bytes, so byte offset 3 lands right after it.
    let pairs = PairsBuilder::new("héllo").rule(Rule::a, 0, 3).build();
    assert_eq!(pairs.peek().unwrap().as_str(), "hé");
}

#[test]
#[should_panic(expected = "invalid span")]
fn out_of_bounds_span_panics() {
    let _ = PairsBuilder::new("ab").rule(Rule::a, 0, 5).build();
}

#[test]
#[should_panic(expected = "invalid span")]
fn non_char_boundary_span_panics() {
    // Byte offset 1 splits the two-byte 'é'.
    let _ = PairsBuilder::new("é").rule(Rule::a, 0, 1).build();
}

#[test]
#[should_panic(expected = "before any rule")]
fn tag_without_rule_panics() {
    let _ = PairsBuilder::<Rule>::new("x").tag("oops");
}
