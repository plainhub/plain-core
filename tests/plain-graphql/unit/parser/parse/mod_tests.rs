use super::*;
#[test]
fn test_number_lookahead_restrictions() {
    GraphQLParser::parse(Rule::ConstList, "[123 abc]").unwrap();
    GraphQLParser::parse(Rule::ConstList, "[123.0123 abc]").unwrap();
    GraphQLParser::parse(Rule::ConstList, "[123.0123e7 abc]").unwrap();
    GraphQLParser::parse(Rule::ConstList, "[123.0123e77 abc]").unwrap();
    assert!(GraphQLParser::parse(Rule::ConstList, "[123abc]").is_err());
    assert!(GraphQLParser::parse(Rule::ConstList, "[123.0123abc]").is_err());
    assert!(GraphQLParser::parse(Rule::ConstList, "[123.0123e7abc]").is_err());
    assert!(GraphQLParser::parse(Rule::ConstList, "[123.0123e77abc]").is_err());
}
