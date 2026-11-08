use super::*;
use std::mem::{offset_of, size_of};

const _: () = {
    const SLOT: usize = size_of::<usize>();
    assert!(offset_of!(Functions, find_class) == 6 * SLOT);
    assert!(offset_of!(Functions, throw_new) == 14 * SLOT);
    assert!(offset_of!(Functions, get_array_length) == 171 * SLOT);
    assert!(offset_of!(Functions, new_byte_array) == 176 * SLOT);
    assert!(offset_of!(Functions, get_byte_array_region) == 200 * SLOT);
    assert!(offset_of!(Functions, set_byte_array_region) == 208 * SLOT);
    assert!(offset_of!(Functions, exception_check) == 228 * SLOT);
};

#[test]
fn exception_messages_encode_nul_and_supplementary_characters() {
    assert_eq!(
        modified_utf8("a\0é😀"),
        b"a\xc0\x80\xc3\xa9\xed\xa0\xbd\xed\xb8\x80\0"
    );
}
