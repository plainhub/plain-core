
    use super::*;

    #[test]
    fn test_is_ascii() {
        assert!(is_ascii_no_nul(b"a"));
        assert!(is_ascii_no_nul(b"abc"));

        assert!(!is_ascii_no_nul(b"\xff"));

        assert!(!is_ascii_no_nul(b"\0"));
        assert!(!is_ascii_no_nul(b"a\0b"));
        assert!(!is_ascii_no_nul(b"ab\0"));
        assert!(!is_ascii_no_nul(b"a\0b\0"));
    }

    #[test]
    #[ignore = "slow, enable this if working on ns_string!"]
    fn test_decode_utf8() {
        for c in '\u{0}'..=core::char::MAX {
            let mut buf;
            for off in 0..4 {
                // Ensure we see garbage if we read outside bounds.
                buf = [0xff; 8];
                let len = c.encode_utf8(&mut buf[off..(off + 4)]).len();
                let (end_idx, decoded) = decode_utf8(&buf, off);
                assert_eq!(
                    (end_idx, decoded),
                    (off + len, c as u32),
                    "failed for U+{code:04X} ({ch:?}) encoded as {buf:#x?} over {range:?}",
                    code = c as u32,
                    ch = c,
                    buf = &buf[off..(off + len)],
                    range = off..(off + len),
                );
            }
        }
    }

    #[test]
    #[ignore = "slow, enable this if working on ns_string!"]
    fn encode_utf16() {
        for c in '\u{0}'..=core::char::MAX {
            assert_eq!(
                c.encode_utf16(&mut [0u16; 2]),
                Utf16Char::encode(c as u32).as_slice(),
                "failed for U+{:04X} ({:?})",
                c as u32,
                c
            );
        }
    }

