use super::*;

#[test]
fn percent_decode_basic() {
    assert_eq!(percent_decode("hello%20world"), "hello world");
    assert_eq!(percent_decode("a+b"), "a b");
    assert_eq!(percent_decode("plain"), "plain");
}

#[test]
fn percent_decode_case_insensitive_hex() {
    assert_eq!(percent_decode("%2F%2f"), "//");
}

#[test]
fn percent_decode_invalid_passthrough() {
    assert_eq!(percent_decode("100%"), "100%");
    assert_eq!(percent_decode("%zz"), "%zz");
    assert_eq!(percent_decode("%2"), "%2");
}

#[test]
fn percent_decode_non_ascii() {
    assert_eq!(percent_decode("caf%C3%A9"), "café");
}

#[test]
fn parse_query_basic() {
    let p = parse_query("id=foo&dl=1");
    assert_eq!(p.get("id").map(String::as_str), Some("foo"));
    assert_eq!(p.get("dl").map(String::as_str), Some("1"));
}

#[test]
fn parse_query_percent_encoded() {
    let p = parse_query("id=hello%20world&x=y%26z");
    assert_eq!(p.get("id").map(String::as_str), Some("hello world"));
    assert_eq!(p.get("x").map(String::as_str), Some("y&z"));
}

#[test]
fn parse_query_handles_plus() {
    let p = parse_query("id=hello+world");
    assert_eq!(p.get("id").map(String::as_str), Some("hello world"));
}

#[test]
fn parse_query_skips_empty_segments() {
    let p = parse_query("&&id=foo&&");
    assert_eq!(p.len(), 1);
    assert_eq!(p.get("id").map(String::as_str), Some("foo"));
}

#[test]
fn parse_query_valueless_key() {
    let p = parse_query("flag");
    assert_eq!(p.get("flag").map(String::as_str), Some(""));
}

#[test]
fn query_get_basic() {
    assert_eq!(query_get("/foo?a=1&b=2", "a").as_deref(), Some("1"));
    assert_eq!(query_get("/foo?a=1&b=2", "b").as_deref(), Some("2"));
}

#[test]
fn query_get_decodes_value() {
    assert_eq!(
        query_get("/x?cid=abc%20def", "cid").as_deref(),
        Some("abc def")
    );
}

#[test]
fn query_get_missing_key() {
    assert!(query_get("/x?a=1", "b").is_none());
}

#[test]
fn query_get_no_query_string() {
    assert!(query_get("/x", "a").is_none());
}

#[test]
fn url_encode_unreserved_passthrough() {
    assert_eq!(url_encode("azAZ09-_.~"), "azAZ09-_.~");
}

#[test]
fn url_encode_spaces_and_specials() {
    assert_eq!(url_encode("hello world"), "hello%20world");
    assert_eq!(url_encode("a/b"), "a%2Fb");
    assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
}

#[test]
fn url_encode_non_ascii_utf8() {
    // café → caf%C3%A9 (UTF-8 bytes 0xC3 0xA9 for é)
    assert_eq!(url_encode("café"), "caf%C3%A9");
    // Chinese characters → %E4%B8%AD%E6%96%87
    assert_eq!(url_encode("中文"), "%E4%B8%AD%E6%96%87");
}

#[test]
fn url_encode_uppercase_hex() {
    assert_eq!(url_encode("\u{00e9}"), "%C3%A9");
}
