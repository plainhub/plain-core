use super::*;
use std::borrow::Cow;
#[test]
fn project_codepages_match_independently_encoded_text() {
    let cases: &[(&Encoding, &[u8], &str)] = &[
        (SHIFT_JIS, &[147, 250, 150, 123, 140, 234], "日本語"),
        (EUC_KR, &[199, 209, 177, 185, 190, 238], "한국어"),
        (BIG5, &[193, 99, 197, 233, 164, 164, 164, 229], "繁體中文"),
        (GBK, &[188, 242, 204, 229, 214, 208, 206, 196], "简体中文"),
        (WINDOWS_874, &[183, 180, 202, 205, 186], "ทดสอบ"),
        (WINDOWS_1250, &[112, 248, 237, 108, 105, 154], "příliš"),
        (WINDOWS_1251, &[239, 240, 232, 236, 229, 240], "пример"),
        (WINDOWS_1253, &[228, 239, 234, 233, 236, 222], "δοκιμή"),
        (WINDOWS_1254, &[246, 114, 110, 101, 107], "örnek"),
        (WINDOWS_1255, &[225, 227, 233, 247, 228], "בדיקה"),
        (WINDOWS_1256, &[199, 206, 202, 200, 199, 209], "اختبار"),
        (WINDOWS_1257, &[226, 98, 111, 108, 115], "ābols"),
        (WINDOWS_1258, &[99, 97, 102, 233], "café"),
        (MACINTOSH, &[99, 97, 102, 142], "café"),
        (
            UTF_8,
            &[
                65, 83, 67, 73, 73, 32, 228, 184, 173, 230, 150, 135, 32, 206, 187,
            ],
            "ASCII 中文 λ",
        ),
        (
            UTF_16LE,
            &[
                65, 0, 83, 0, 67, 0, 73, 0, 73, 0, 32, 0, 45, 78, 135, 101, 32, 0, 187, 3,
            ],
            "ASCII 中文 λ",
        ),
        (
            UTF_16BE,
            &[
                0, 65, 0, 83, 0, 67, 0, 73, 0, 73, 0, 32, 78, 45, 101, 135, 0, 32, 3, 187,
            ],
            "ASCII 中文 λ",
        ),
    ];
    for &(encoding, bytes, expected) in cases {
        assert_eq!(
            encoding
                .decode_without_bom_handling_and_without_replacement(bytes)
                .as_deref(),
            Some(expected)
        );
        let (decoded, _, errors) = encoding.decode(bytes);
        assert!(!errors);
        assert_eq!(decoded, expected);
    }
}
#[test]
fn bom_sniffing_keeps_existing_encoding_override_and_removal() {
    for (bytes, expected) in [
        (b"\xEF\xBB\xBFtext".as_slice(), "text"),
        (b"\xFF\xFEt\0e\0x\0t\0".as_slice(), "text"),
        (b"\xFE\xFF\0t\0e\0x\0t".as_slice(), "text"),
    ] {
        for encoding in [UTF_8, GBK, BIG5, WINDOWS_1251] {
            let (decoded, _, errors) = encoding.decode(bytes);
            assert!(!errors);
            assert_eq!(decoded, expected);
        }
    }
    assert_eq!(
        UTF_8
            .decode_without_bom_handling_and_without_replacement(b"\xEF\xBB\xBFtext")
            .unwrap(),
        "\u{FEFF}text"
    );
}
#[test]
fn malformed_trailing_sequences_preserve_strict_and_lossy_contracts() {
    for (encoding, bytes) in [
        (GBK, b"\xC4".as_slice()),
        (BIG5, b"\xA4".as_slice()),
        (SHIFT_JIS, b"\x82".as_slice()),
        (EUC_KR, b"\xC7".as_slice()),
        (UTF_8, b"\xF0\x9F\x92".as_slice()),
        (UTF_16LE, b"\x41".as_slice()),
        (UTF_16BE, b"\x41".as_slice()),
    ] {
        assert!(
            encoding
                .decode_without_bom_handling_and_without_replacement(bytes)
                .is_none()
        );
        let (decoded, _, errors) = encoding.decode(bytes);
        assert!(errors);
        assert_eq!(decoded, "\u{FFFD}");
    }
}
#[test]
fn ascii_and_valid_utf8_keep_borrowed_storage() {
    let ascii = b"independent ASCII text";
    for encoding in [UTF_8, GBK, BIG5, WINDOWS_1251] {
        let decoded = encoding
            .decode_without_bom_handling_and_without_replacement(ascii)
            .unwrap();
        assert!(matches!(decoded, Cow::Borrowed(_)));
        assert_eq!(decoded.as_ptr(), ascii.as_ptr());
    }
    let input = "中文 λ 한국어".repeat(64);
    let decoded = UTF_8
        .decode_without_bom_handling_and_without_replacement(input.as_bytes())
        .unwrap();
    assert!(matches!(decoded, Cow::Borrowed(_)));
    assert_eq!(decoded.as_ptr(), input.as_ptr());
}
#[test]
fn utf8_simd_and_scalar_error_boundaries_match_the_standard_library() {
    let original = "ASCII 中文 日本語 한국어 café λ ".repeat(6).into_bytes();
    for at in 0..original.len() {
        for value in [0, 0x7F, 0x80, 0xBF, 0xC0, 0xF5, 0xFF] {
            let mut bytes = original.clone();
            bytes[at] = value;
            let strict = UTF_8.decode_without_bom_handling_and_without_replacement(&bytes);
            assert_eq!(
                strict.as_deref(),
                std::str::from_utf8(&bytes).ok(),
                "byte {at}"
            );
            let (lossy, errors) = UTF_8.decode_without_bom_handling(&bytes);
            assert_eq!(lossy, String::from_utf8_lossy(&bytes), "byte {at}");
            assert_eq!(errors, std::str::from_utf8(&bytes).is_err());
        }
    }
}
