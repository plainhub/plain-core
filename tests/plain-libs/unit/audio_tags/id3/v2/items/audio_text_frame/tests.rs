use crate::audio_tags::TextEncoding;
use crate::audio_tags::id3::v2::{AudioTextFrame, AudioTextFrameFlags, FrameFlags};

fn expected() -> AudioTextFrame<'static> {
    AudioTextFrame {
        header: super::FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        encoding: TextEncoding::Latin1,
        mime_type: String::from("audio/mpeg"),
        flags: AudioTextFrameFlags { scrambling: false },
        equivalent_text: String::from("foo bar baz"),
        audio_data: vec![0xff, 0xfb, 0x90, 0x64, 0x00, 0x7f, 0x80, 0xff],
    }
}

#[test]
fn atxt_decode() {
    let expected = expected();

    let cont = crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.atxt");

    let parsed_atxt = AudioTextFrame::parse(&cont, FrameFlags::default()).unwrap();

    assert_eq!(parsed_atxt, expected);
}

#[test]
fn atxt_encode() {
    let to_encode = expected();

    let encoded = to_encode.as_bytes();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.atxt");

    assert_eq!(encoded, expected_bytes);
}
