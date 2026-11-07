use super::super::super::Parser;
use super::super::super::macros::tests::*;
use alloc::vec;
use alloc::vec::Vec;

#[test]
fn iter_for_flat_pairs() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();

    assert_eq!(
        pairs.flatten().map(|p| p.as_rule()).collect::<Vec<Rule>>(),
        vec![Rule::a, Rule::b, Rule::c]
    );
}

#[test]
fn double_ended_iter_for_flat_pairs() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();
    assert_eq!(
        pairs
            .flatten()
            .rev()
            .map(|p| p.as_rule())
            .collect::<Vec<Rule>>(),
        vec![Rule::c, Rule::b, Rule::a]
    );
}

#[test]
fn test_line_col() {
    let mut pairs = AbcParser::parse(Rule::a, "abcNe\nabcde").unwrap().flatten();

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "abc");
    assert_eq!(pair.line_col(), (1, 1));
    assert_eq!(pair.line_col(), pair.as_span().start_pos().line_col());

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "b");
    assert_eq!(pair.line_col(), (1, 2));
    assert_eq!(pair.line_col(), pair.as_span().start_pos().line_col());

    let pair = pairs.next().unwrap();
    assert_eq!(pair.as_str(), "e");
    assert_eq!(pair.line_col(), (1, 5));
    assert_eq!(pair.line_col(), pair.as_span().start_pos().line_col());
}

#[test]
fn exact_size_iter_for_pairs() {
    let pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap().flatten();
    assert_eq!(pairs.len(), pairs.count());

    let pairs = AbcParser::parse(Rule::a, "我很漂亮efgh").unwrap().flatten();
    assert_eq!(pairs.len(), pairs.count());

    let pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap().flatten();
    let pairs = pairs.rev();
    assert_eq!(pairs.len(), pairs.count());

    let mut pairs = AbcParser::parse(Rule::a, "abc\nefgh").unwrap().flatten();
    let pairs_len = pairs.len();
    let _ = pairs.next().unwrap();
    assert_eq!(pairs.count() + 1, pairs_len);
}
