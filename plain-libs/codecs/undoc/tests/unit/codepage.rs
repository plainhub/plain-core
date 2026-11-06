use super::*;

#[test]
fn windows_1252_maps_the_windows_block_and_passes_latin1_through() {
    assert_eq!(windows_1252(b'A'), 'A');
    assert_eq!(windows_1252(0x93), '\u{201C}');
    assert_eq!(windows_1252(0x80), '\u{20AC}');
    assert_eq!(windows_1252(0xE9), 'é');
    assert_eq!(decode_strict(&[0x80], WINDOWS_ANSI).as_deref(), Some("€"));
}

#[cfg(feature = "xls")]
#[test]
fn strict_leaves_out_what_lossy_marks() {
    // An unknown code page: strict refuses the non-ASCII byte, lossy marks it.
    assert_eq!(decode_strict(b"ab\xFF", 4242), None);
    assert_eq!(decode_lossy(b"ab\xFF", 4242), "ab\u{FFFD}");
    assert_eq!(decode_lossy(b"plain", 4242), "plain");
}

#[cfg(all(feature = "codepages", feature = "xls"))]
#[test]
fn multi_byte_code_pages_decode_with_tables() {
    let euc_kr = [0xBA, 0xB8, 0xB0, 0xED, 0xBC, 0xAD]; // 보고서
    assert_eq!(decode_strict(&euc_kr, 949).as_deref(), Some("보고서"));
    // A truncated double-byte character: strict refuses, lossy keeps the rest.
    assert_eq!(decode_strict(&euc_kr[..5], 949), None);
    assert_eq!(decode_lossy(&euc_kr[..5], 949), "보고\u{FFFD}");
    assert_eq!(
        decode_strict(b"caf\x8E", APPLE_ROMAN).as_deref(),
        Some("café")
    );
}

#[cfg(feature = "xls")]
#[test]
fn charsets_name_their_code_pages() {
    assert_eq!(from_charset(129), Some(949));
    assert_eq!(from_charset(0), None);
    assert_eq!(from_charset(2), None);
}
