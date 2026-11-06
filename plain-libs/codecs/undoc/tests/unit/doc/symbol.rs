use super::*;

#[test]
fn symbol_font_codes_map_through_the_symbol_encoding() {
    assert_eq!(resolve(Some("Symbol"), 0xF061), Some('α'));
    assert_eq!(resolve(Some("symbol"), 0xF028), Some('('));
    assert_eq!(resolve(Some("Symbol"), 0xF0B1), Some('±'));
}

#[test]
fn other_symbol_fonts_keep_the_code_word_stores() {
    assert_eq!(resolve(Some("Wingdings"), 0xF0A7), Some('\u{F0A7}'));
    assert_eq!(resolve(None, 0xF061), Some('\u{F061}'));
    assert_eq!(resolve(Some("Symbol"), 0x00B1), Some('±'));
}
