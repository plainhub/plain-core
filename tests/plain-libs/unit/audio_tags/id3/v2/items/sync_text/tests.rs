use crate::audio_tags::id3::v2::{
    FrameFlags, FrameHeader, SyncTextContentType, SynchronizedTextFrame, TimestampFormat,
};
use crate::audio_tags::util::text::TextEncoding;

fn expected(encoding: TextEncoding) -> SynchronizedTextFrame<'static> {
    SynchronizedTextFrame {
        header: FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        encoding,
        language: *b"eng",
        timestamp_format: TimestampFormat::MS,
        content_type: SyncTextContentType::Lyrics,
        description: Some(String::from("Test Sync Text")),
        content: vec![
            (0, String::from("\nLofty")),
            (10000, String::from("\nIs")),
            (15000, String::from("\nReading")),
            (30000, String::from("\nThis")),
            (1_938_000, String::from("\nCorrectly")),
        ],
    }
}

#[test]
fn sylt_decode() {
    let cont = crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.sylt");

    let parsed_sylt = SynchronizedTextFrame::parse(&cont, FrameFlags::default()).unwrap();

    assert_eq!(parsed_sylt, expected(TextEncoding::Latin1));
}

#[test]
fn sylt_encode() {
    let encoded = expected(TextEncoding::Latin1).as_bytes().unwrap();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.sylt");

    assert_eq!(encoded, expected_bytes);
}

#[test]
fn sylt_decode_utf16() {
    let cont = crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test_utf16.sylt");

    let parsed_sylt = SynchronizedTextFrame::parse(&cont, FrameFlags::default()).unwrap();

    assert_eq!(parsed_sylt, expected(TextEncoding::UTF16));
}

#[test]
fn sylt_encode_utf_16() {
    let encoded = expected(TextEncoding::UTF16).as_bytes().unwrap();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test_utf16.sylt");

    assert_eq!(encoded, expected_bytes);
}
