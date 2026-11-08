use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, PrivateFrame};

fn expected() -> PrivateFrame<'static> {
    PrivateFrame {
        header: FrameHeader::new(super::FRAME_ID, FrameFlags::default()),
        owner: String::from("unit\x40x.test"),
        private_data: String::from("some data").into_bytes(),
    }
}

#[test]
fn priv_decode() {
    let cont = crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.priv");

    let parsed_priv = PrivateFrame::parse(&mut &cont[..], FrameFlags::default())
        .unwrap()
        .unwrap();

    assert_eq!(parsed_priv, expected());
}

#[test]
fn priv_encode() {
    let encoded = expected().as_bytes().unwrap();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.priv");

    assert_eq!(encoded, expected_bytes);
}
