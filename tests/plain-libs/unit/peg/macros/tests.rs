use super::super::error::Error;
use super::super::iterators::Pairs;
use super::super::{Parser, state};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rule {
    a,
    b,
    c,
    d,
}

pub struct AbcParser;

impl Parser<Rule> for AbcParser {
    fn parse(_: Rule, input: &str) -> Result<Pairs<'_, Rule>, Error<Rule>> {
        state(input, |state| {
            state
                .rule(Rule::a, |s| {
                    s.skip(1)
                        .unwrap()
                        .rule(Rule::b, |s| s.skip(1))
                        .unwrap()
                        .skip(1)
                })
                .and_then(|s| s.skip(1).unwrap().rule(Rule::c, |s| s.match_string("e")))
                .and_then(|s| s.optional(|s| s.rule(Rule::d, |s| s.match_string("fgh"))))
        })
    }
}

#[test]
fn parses_to() {
    parses_to! {
        parser: AbcParser,
        input: "abcde",
        rule: Rule::a,
        tokens: [
            a(0, 3, [
                b(1, 2),
            ]),
            c(4, 5)
        ]
    };
}

#[test]
#[should_panic]
fn missing_end() {
    parses_to! {
        parser: AbcParser,
        input: "abcde",
        rule: Rule::a,
        tokens: [
            a(0, 3, [
                b(1, 2)
            ])
        ]
    };
}

#[test]
#[should_panic]
fn empty() {
    parses_to! {
        parser: AbcParser,
        input: "abcde",
        rule: Rule::a,
        tokens: []
    };
}

#[test]
fn fails_with() {
    fails_with! {
        parser: AbcParser,
        input: "abcdf",
        rule: Rule::a,
        positives: vec![Rule::c],
        negatives: vec![],
        pos: 4
    };
}

#[test]
#[should_panic]
fn wrong_positives() {
    fails_with! {
        parser: AbcParser,
        input: "abcdf",
        rule: Rule::a,
        positives: vec![Rule::a],
        negatives: vec![],
        pos: 4
    };
}

#[test]
#[should_panic]
fn wrong_negatives() {
    fails_with! {
        parser: AbcParser,
        input: "abcdf",
        rule: Rule::a,
        positives: vec![Rule::c],
        negatives: vec![Rule::c],
        pos: 4
    };
}

#[test]
#[should_panic]
fn wrong_pos() {
    fails_with! {
        parser: AbcParser,
        input: "abcdf",
        rule: Rule::a,
        positives: vec![Rule::c],
        negatives: vec![],
        pos: 3
    };
}
