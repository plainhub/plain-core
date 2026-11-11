use super::*;
#[test]
fn unicode_and_capture_offsets_keep_utf8_boundaries() {
    let re = compile(r"(?P<word>\p{Greek}+)\s+(\d+)").unwrap();
    let text = "前 αβ 123 后";
    let cap = captures(&re, text).unwrap();
    assert_eq!(&text[cap.get_group_by_name("word").unwrap().range()], "αβ");
    assert_eq!(&text[cap.get_group(2).unwrap().range()], "123");
    assert_eq!(re.find(text).unwrap().start(), 4);
}
#[test]
fn invalid_patterns_and_oversized_automata_remain_errors() {
    for pattern in ["(", r"(?-u:\xFF)", "a{999999999}"] {
        assert!(compile(pattern).is_err(), "{pattern}");
    }
    let re = compile(r"\b\w+\b").unwrap();
    assert!(re.is_match("中文"));
    assert!(!compile(r"^a+$").unwrap().is_match("aaab"));
}
