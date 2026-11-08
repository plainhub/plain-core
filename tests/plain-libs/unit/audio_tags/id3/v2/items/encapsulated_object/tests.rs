use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, GeneralEncapsulatedObject};
use crate::audio_tags::util::text::TextEncoding;

fn expected() -> GeneralEncapsulatedObject<'static> {
    GeneralEncapsulatedObject {
        header: FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        encoding: TextEncoding::Latin1,
        mime_type: Some(String::from("audio/mpeg")),
        file_name: Some(String::from("a.mp3")),
        descriptor: Some(String::from("Test Asset")),
        data: vec![0xff, 0xfb, 0x90, 0x64, 0x00, 0x7f, 0x80, 0xff],
    }
}

#[test]
fn geob_decode() {
    let expected = expected();

    let cont = crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.geob");

    let parsed_geob = GeneralEncapsulatedObject::parse(&cont, FrameFlags::default()).unwrap();

    assert_eq!(parsed_geob, expected);
}

#[test]
fn geob_encode() {
    let to_encode = expected();

    let encoded = to_encode.as_bytes();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.geob");

    assert_eq!(encoded, expected_bytes);
}
