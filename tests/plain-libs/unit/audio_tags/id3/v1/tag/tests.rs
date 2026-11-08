use crate::audio_tags::id3::v1::Id3v1Tag;
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::{Tag, TagType};

#[test]
fn parse_id3v1() {
    let expected_tag = Id3v1Tag {
        title: Some(String::from("Foo title")),
        artist: Some(String::from("Bar artist")),
        album: Some(String::from("Baz album")),
        year: Some(String::from("1984")),
        comment: Some(String::from("Qux comment")),
        track_number: Some(1),
        genre: Some(32),
    };

    let tag = crate::audio_tags::tag::utils::read_tag_fixture("test.id3v1");
    let parsed_tag = crate::audio_tags::id3::v1::read::parse_id3v1(tag.try_into().unwrap());

    assert_eq!(expected_tag, parsed_tag);
}

#[test]
fn id3v1_to_tag() {
    let tag_bytes = crate::audio_tags::tag::utils::read_tag_fixture("test.id3v1");
    let id3v1 = crate::audio_tags::id3::v1::read::parse_id3v1(tag_bytes.try_into().unwrap());

    let tag: Tag = id3v1.into();

    crate::audio_tags::tag::utils::verify_tag(&tag, true, true);
}

#[test]
fn tag_to_id3v1() {
    let tag = crate::audio_tags::tag::utils::create_tag(TagType::Id3v1);

    let id3v1_tag: Id3v1Tag = tag.into();

    assert_eq!(id3v1_tag.title.as_deref(), Some("Foo title"));
    assert_eq!(id3v1_tag.artist.as_deref(), Some("Bar artist"));
    assert_eq!(id3v1_tag.album.as_deref(), Some("Baz album"));
    assert_eq!(id3v1_tag.comment.as_deref(), Some("Qux comment"));
    assert_eq!(id3v1_tag.track_number, Some(1));
    assert_eq!(id3v1_tag.genre, Some(32));
}
