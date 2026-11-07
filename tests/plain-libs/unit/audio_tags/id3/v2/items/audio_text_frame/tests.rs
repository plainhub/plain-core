use crate::audio_tags::TextEncoding;
use crate::audio_tags::id3::v2::{AudioTextFrame, AudioTextFrameFlags, FrameFlags};

fn expected() -> AudioTextFrame<'static> {
    AudioTextFrame {
        header: super::FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        encoding: TextEncoding::Latin1,
        mime_type: String::from("audio/mpeg"),
        flags: AudioTextFrameFlags { scrambling: false },
        equivalent_text: String::from("foo bar baz"),
        audio_data: crate::audio_tags::tag::utils::read_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/minimal/full_test.mp3"
        )),
    }
}

#[test]
fn atxt_decode() {
    let expected = expected();

    let cont = crate::audio_tags::tag::utils::read_path("tests/tags/assets/id3v2/test.atxt");

    let parsed_atxt = AudioTextFrame::parse(&cont, FrameFlags::default()).unwrap();

    assert_eq!(parsed_atxt, expected);
}

#[test]
fn atxt_encode() {
    let to_encode = expected();

    let encoded = to_encode.as_bytes();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_path("tests/tags/assets/id3v2/test.atxt");

    assert_eq!(encoded, expected_bytes);
}
