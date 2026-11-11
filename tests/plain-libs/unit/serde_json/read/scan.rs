use super::*;

thread_local! { static BLOCKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub(super) fn record_block() {
    BLOCKS.set(BLOCKS.get() + 1);
}

#[test]
fn vector_scan_matches_each_byte_and_unaligned_tail_boundary() {
    let supported = cfg!(any(
        all(target_arch = "aarch64", target_feature = "neon"),
        target_arch = "x86_64"
    ));
    for byte in 0..=255_u8 {
        for position in 0..32 {
            let mut input = [b'a'; 32];
            input[position] = byte;
            let expected = input
                .iter()
                .position(|&b| b < 0x20 || b == b'"' || b == b'\\')
                .unwrap_or(input.len());
            assert_eq!(
                find_escape(&input),
                supported.then_some(expected),
                "byte={byte}, position={position}"
            );
        }
    }
    for alignment in 0..16 {
        for len in 0..80 {
            for position in 0..=len {
                for byte in [0, 0x1f, b'"', b'\\', 0x80, b'a'] {
                    let mut input = vec![b'a'; alignment + len + 16];
                    if position < len {
                        input[alignment + position] = byte;
                    }
                    let bytes = &input[alignment..alignment + len];
                    let expected = bytes
                        .iter()
                        .position(|&b| b < 0x20 || b == b'"' || b == b'\\')
                        .unwrap_or(len);
                    assert_eq!(
                        find_escape(bytes),
                        (supported && len >= 16).then_some(expected)
                    );
                }
            }
        }
    }
}

#[test]
fn public_string_parse_uses_vectors_without_copying_or_accepting_invalid_utf8() {
    let mut input = vec![b'"'];
    input.extend_from_slice(b"abcdefghijklmnopqrstuvwxyz0123456789");
    input.push(b'"');
    BLOCKS.set(0);
    let value: &str = crate::serde_json::from_slice(&input).unwrap();
    assert_eq!(value.as_ptr(), input[1..].as_ptr());
    if cfg!(any(
        all(target_arch = "aarch64", target_feature = "neon"),
        target_arch = "x86_64"
    )) {
        assert!(BLOCKS.get() > 0);
    }
    for position in 1..input.len() - 1 {
        for byte in [0, 0x1f, 0x80, 0xff] {
            let mut invalid = input.clone();
            invalid[position] = byte;
            assert!(crate::serde_json::from_slice::<&str>(&invalid).is_err());
        }
    }
}
