use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::iff::chunk::Chunks;
use crate::audio_tags::iff::wav::RiffInfoList;
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::{Tag, TagType};

use crate::binary::LittleEndian;

use std::io::Cursor;

#[test]
fn parse_riff_info() {
    let mut expected_tag = RiffInfoList::default();

    expected_tag.insert(String::from("IART"), String::from("Bar artist"));
    expected_tag.insert(String::from("ICMT"), String::from("Qux comment"));
    expected_tag.insert(String::from("ICRD"), String::from("1984"));
    expected_tag.insert(String::from("INAM"), String::from("Foo title"));
    expected_tag.insert(String::from("IPRD"), String::from("Baz album"));
    expected_tag.insert(String::from("IPRT"), String::from("1"));

    let tag = crate::audio_tags::tag::utils::read_tag_fixture("test.riff");
    let mut parsed_tag = RiffInfoList::default();

    super::read::parse_riff_info(
        &mut Cursor::new(&tag[..]),
        &mut Chunks::<LittleEndian>::new(tag.len() as u64),
        (tag.len() - 1) as u64,
        &mut parsed_tag,
        ParsingMode::Strict,
    )
    .unwrap();

    assert_eq!(expected_tag, parsed_tag);
}

#[test]
fn riff_info_to_tag() {
    let tag_bytes = crate::audio_tags::tag::utils::read_tag_fixture("test.riff");

    let mut reader = std::io::Cursor::new(&tag_bytes[..]);
    let mut riff_info = RiffInfoList::default();

    super::read::parse_riff_info(
        &mut reader,
        &mut Chunks::<LittleEndian>::new(tag_bytes.len() as u64),
        (tag_bytes.len() - 1) as u64,
        &mut riff_info,
        ParsingMode::Strict,
    )
    .unwrap();

    let tag: Tag = riff_info.into();

    crate::audio_tags::tag::utils::verify_tag(&tag, true, false);
}

#[test]
fn tag_to_riff_info() {
    let tag = crate::audio_tags::tag::utils::create_tag(TagType::RiffInfo);

    let riff_info: RiffInfoList = tag.into();

    assert_eq!(riff_info.get("INAM"), Some("Foo title"));
    assert_eq!(riff_info.get("IART"), Some("Bar artist"));
    assert_eq!(riff_info.get("IPRD"), Some("Baz album"));
    assert_eq!(riff_info.get("ICMT"), Some("Qux comment"));
    assert_eq!(riff_info.get("IPRT"), Some("1"));
}
