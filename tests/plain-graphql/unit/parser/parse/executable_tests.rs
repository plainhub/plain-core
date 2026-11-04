use super::*;
use std::fs;
#[test]
fn test_parser() {
    for entry in fs::read_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plain-rs/testdata/graphql/executables"
    ))
    .unwrap()
    {
        let entry = entry.unwrap();
        eprintln!("Parsing file {}", entry.path().display());
        GraphQLParser::parse(
            Rule::ExecutableDocument,
            &fs::read_to_string(entry.path()).unwrap(),
        )
        .unwrap();
    }
}
#[test]
fn test_parser_ast() {
    for entry in fs::read_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plain-rs/testdata/graphql/executables"
    ))
    .unwrap()
    {
        let entry = entry.unwrap();
        eprintln!("Parsing and transforming file {}", entry.path().display());
        parse_query(fs::read_to_string(entry.path()).unwrap()).unwrap();
    }
}
#[test]
fn test_parse_overflowing_int() {
    let query_ok = format!("mutation {{ add(big: {}) }} ", i32::MAX);
    let query_overflow = format!("mutation {{ add(big: {}0000) }} ", i32::MAX);
    assert!(parse_query(query_ok).is_ok());
    assert!(parse_query(query_overflow).is_ok());
}
