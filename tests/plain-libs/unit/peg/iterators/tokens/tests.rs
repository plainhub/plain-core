use super::super::super::Parser;
use super::super::super::macros::tests::*;
use super::Token;
use alloc::vec::Vec;

#[test]
fn double_ended_iter_for_tokens() {
    let pairs = AbcParser::parse(Rule::a, "abcde").unwrap();
    let mut tokens = pairs.clone().tokens().collect::<Vec<Token<'_, Rule>>>();
    tokens.reverse();
    let reverse_tokens = pairs.tokens().rev().collect::<Vec<Token<'_, Rule>>>();
    assert_eq!(tokens, reverse_tokens);
}

#[test]
fn exact_size_iter_for_tokens() {
    let tokens = AbcParser::parse(Rule::a, "abcde").unwrap().tokens();
    assert_eq!(tokens.len(), tokens.count());

    let tokens = AbcParser::parse(Rule::a, "我很漂亮e").unwrap().tokens();
    assert_eq!(tokens.len(), tokens.count());

    let tokens = AbcParser::parse(Rule::a, "abcde").unwrap().tokens().rev();
    assert_eq!(tokens.len(), tokens.count());

    let mut tokens = AbcParser::parse(Rule::a, "abcde").unwrap().tokens();
    let tokens_len = tokens.len();
    let _ = tokens.next().unwrap();
    assert_eq!(tokens.count() + 1, tokens_len);
}
