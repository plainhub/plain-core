use super::registry_string;

#[test]
fn registry_strings_preserve_utf16_and_trim_only_trailing_nuls() {
    let bytes: Vec<_> = "Windows 😀\0\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(registry_string(&bytes).as_deref(), Some("Windows 😀"));
    assert_eq!(registry_string(&[0, 0, b'A', 0]).as_deref(), Some("\0A"));
    assert_eq!(registry_string(&[b'A', 0]).as_deref(), Some("A"));
}

#[test]
fn registry_strings_reject_partial_words_without_reinterpreting_unaligned_memory() {
    assert_eq!(registry_string(&[b'A']), None);
    let bytes = [255, b'A', 0, 0, 0];
    assert_eq!(registry_string(&bytes[1..]).as_deref(), Some("A"));
    assert_eq!(registry_string(&[0, 216]).as_deref(), Some("�"));
}
