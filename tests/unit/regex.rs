use super::compile;

#[test]
fn packed_literal_search_and_capture_offsets_match_byte_positions() {
    let regex =
        compile("(?P<item>alpha|alphabet|bravo|charlie|delta|echo|foxtrot|golf|hotel)").unwrap();
    for offset in 0..128 {
        for word in [
            "alpha", "alphabet", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
        ] {
            let prefix = "中".repeat(offset);
            let text = format!("{prefix}{word}末尾");
            let found = regex.find(&text).unwrap();
            let expected = if word == "alphabet" { "alpha" } else { word };
            assert_eq!(found.range(), prefix.len()..prefix.len() + expected.len());
            let captures = super::captures(&regex, &text).unwrap();
            assert_eq!(
                captures.get_group_by_name("item").unwrap().range(),
                found.range()
            );
        }
        assert!(!regex.is_match(&"中".repeat(offset)));
    }
}
