use crate::audio_tags::util::text::{TextDecodeOptions, TextEncoding};
use std::io::Cursor;

const TEST_STRING: &str = "l\u{00f8}ft\u{00a5}";

#[test]
fn text_decode() {
    // No BOM
    let utf16_decode = super::utf16_decode_bytes(
        &[
            0x00, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5, 0x00, 0x00,
        ],
        u16::from_be_bytes,
    )
    .unwrap();

    assert_eq!(utf16_decode, TEST_STRING.to_string());

    // BOM test
    let be_utf16_decode = super::decode_text(
        &mut Cursor::new(&[
            0xFE, 0xFF, 0x00, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5, 0x00, 0x00,
        ]),
        TextDecodeOptions::new().encoding(TextEncoding::UTF16),
    )
    .unwrap();
    let le_utf16_decode = super::decode_text(
        &mut Cursor::new(&[
            0xFF, 0xFE, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5, 0x00, 0x00, 0x00,
        ]),
        TextDecodeOptions::new().encoding(TextEncoding::UTF16),
    )
    .unwrap();

    assert_eq!(be_utf16_decode.content, le_utf16_decode.content);
    assert_eq!(be_utf16_decode.bytes_read, le_utf16_decode.bytes_read);
    assert_eq!(be_utf16_decode.content, TEST_STRING.to_string());

    let utf8_decode = super::decode_text(
        &mut TEST_STRING.as_bytes(),
        TextDecodeOptions::new().encoding(TextEncoding::UTF8),
    )
    .unwrap();

    assert_eq!(utf8_decode.content, TEST_STRING.to_string());
}

#[test]
fn text_encode() {
    // No BOM
    let utf16_encode = super::utf16_encode(TEST_STRING, u16::to_be_bytes, true, false);

    assert_eq!(
        utf16_encode.as_slice(),
        &[
            0xFE, 0xFF, 0x00, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5
        ]
    );

    // BOM test
    let be_utf16_encode = super::encode_text(TEST_STRING, TextEncoding::UTF16BE, false);
    let le_utf16_encode = super::utf16_encode(TEST_STRING, u16::to_le_bytes, true, false);
    let be_utf16_encode_bom = super::utf16_encode(TEST_STRING, u16::to_be_bytes, true, false);

    assert_ne!(be_utf16_encode.as_slice(), le_utf16_encode.as_slice());
    // TextEncoding::UTF16BE has no BOM
    assert_eq!(
        be_utf16_encode.as_slice(),
        &[0x00, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5]
    );
    assert_eq!(
        le_utf16_encode.as_slice(),
        &[
            0xFF, 0xFE, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5, 0x00
        ]
    );
    assert_eq!(
        be_utf16_encode_bom.as_slice(),
        &[
            0xFE, 0xFF, 0x00, 0x6C, 0x00, 0xF8, 0x00, 0x66, 0x00, 0x74, 0x00, 0xA5
        ]
    );

    let utf8_encode = super::encode_text(TEST_STRING, TextEncoding::UTF8, false);

    assert_eq!(utf8_encode.as_slice(), TEST_STRING.as_bytes());
}
