use crate::audio_tags::TextEncoding;
use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId, OwnershipFrame};

use std::borrow::Cow;

fn expected() -> OwnershipFrame<'static> {
    OwnershipFrame {
        header: FrameHeader::new(FrameId::Valid(Cow::Borrowed("OWNE")), FrameFlags::default()),
        encoding: TextEncoding::Latin1,
        price_paid: String::from("USD1000"),
        date_of_purchase: String::from("19840407"),
        seller: String::from("FooBar"),
    }
}

#[test]
fn owne_decode() {
    let cont = crate::audio_tags::tag::utils::read_path("tests/tags/assets/id3v2/test.owne");

    let parsed_owne = OwnershipFrame::parse(&mut &cont[..], FrameFlags::default())
        .unwrap()
        .unwrap();

    assert_eq!(parsed_owne, expected());
}

#[test]
fn owne_encode() {
    let encoded = expected().as_bytes(false).unwrap();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_path("tests/tags/assets/id3v2/test.owne");

    assert_eq!(encoded, expected_bytes);
}
