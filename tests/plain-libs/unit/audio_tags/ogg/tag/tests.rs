use crate::audio_tags::config::{ParseOptions, ParsingMode};
use crate::audio_tags::ogg::{OggPictureStorage, VorbisComments};
use crate::audio_tags::picture::{MimeType, Picture, PictureType};
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::{ItemValue, Tag, TagItem, TagType};
use std::io::Cursor;

fn read_tag(tag: &[u8]) -> VorbisComments {
    let mut reader = std::io::Cursor::new(tag);

    crate::audio_tags::ogg::read::read_comments(
        &mut reader,
        tag.len() as u64,
        ParseOptions::new().parsing_mode(ParsingMode::Strict),
    )
    .unwrap()
}

#[test]
fn parse_vorbis_comments() {
    let mut expected_tag = VorbisComments::default();

    expected_tag.set_vendor(String::from("Lavf58.76.100"));

    expected_tag.push(String::from("ALBUM"), String::from("Baz album"));
    expected_tag.push(String::from("ARTIST"), String::from("Bar artist"));
    expected_tag.push(String::from("COMMENT"), String::from("Qux comment"));
    expected_tag.push(String::from("DATE"), String::from("1984"));
    expected_tag.push(String::from("GENRE"), String::from("Classical"));
    expected_tag.push(String::from("TITLE"), String::from("Foo title"));
    expected_tag.push(String::from("TRACKNUMBER"), String::from("1"));

    let file_cont = crate::audio_tags::tag::utils::read_path("tests/tags/assets/test.vorbis");
    let parsed_tag = read_tag(&file_cont);

    assert_eq!(expected_tag, parsed_tag);
}

#[test]
fn vorbis_comments_to_tag() {
    let tag_bytes = std::fs::read("tests/tags/assets/test.vorbis").unwrap();
    let vorbis_comments = read_tag(&tag_bytes);

    let tag: Tag = vorbis_comments.into();

    crate::audio_tags::tag::utils::verify_tag(&tag, true, true);
}

#[test]
fn tag_to_vorbis_comments() {
    let tag = crate::audio_tags::tag::utils::create_tag(TagType::VorbisComments);

    let vorbis_comments: VorbisComments = tag.into();

    assert_eq!(vorbis_comments.get("TITLE"), Some("Foo title"));
    assert_eq!(vorbis_comments.get("ARTIST"), Some("Bar artist"));
    assert_eq!(vorbis_comments.get("ALBUM"), Some("Baz album"));
    assert_eq!(vorbis_comments.get("COMMENT"), Some("Qux comment"));
    assert_eq!(vorbis_comments.get("TRACKNUMBER"), Some("1"));
    assert_eq!(vorbis_comments.get("GENRE"), Some("Classical"));
}

#[test]
fn multi_value_roundtrip() {
    let mut tag = Tag::new(TagType::VorbisComments);
    tag.insert_text(ItemKey::TrackArtist, "TrackArtist 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::TrackArtist,
        ItemValue::Text("TrackArtist 2".to_owned()),
    ));
    tag.insert_text(ItemKey::AlbumArtist, "AlbumArtist 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::AlbumArtist,
        ItemValue::Text("AlbumArtist 2".to_owned()),
    ));
    tag.insert_text(ItemKey::TrackTitle, "TrackTitle 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::TrackTitle,
        ItemValue::Text("TrackTitle 2".to_owned()),
    ));
    tag.insert_text(ItemKey::AlbumTitle, "AlbumTitle 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::AlbumTitle,
        ItemValue::Text("AlbumTitle 2".to_owned()),
    ));
    tag.insert_text(ItemKey::Comment, "Comment 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::Comment,
        ItemValue::Text("Comment 2".to_owned()),
    ));
    tag.insert_text(ItemKey::ContentGroup, "ContentGroup 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::ContentGroup,
        ItemValue::Text("ContentGroup 2".to_owned()),
    ));
    tag.insert_text(ItemKey::Genre, "Genre 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::Genre,
        ItemValue::Text("Genre 2".to_owned()),
    ));
    tag.insert_text(ItemKey::Mood, "Mood 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::Mood,
        ItemValue::Text("Mood 2".to_owned()),
    ));
    tag.insert_text(ItemKey::Composer, "Composer 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::Composer,
        ItemValue::Text("Composer 2".to_owned()),
    ));
    tag.insert_text(ItemKey::Conductor, "Conductor 1".to_owned());
    tag.push(TagItem::new(
        ItemKey::Conductor,
        ItemValue::Text("Conductor 2".to_owned()),
    ));
    // Otherwise the following item would be inserted implicitly
    // during the conversion.
    tag.insert_text(ItemKey::EncoderSoftware, "EncoderSoftware".to_owned());
    assert_eq!(20 + 1, tag.len());

    let mut vorbis_comments1 = VorbisComments::from(tag.clone());

    let (split_remainder, split_tag) = vorbis_comments1.clone().split_tag();
    assert_eq!(0, split_remainder.len());
    assert_eq!(tag.len(), split_tag.len());

    // Merge back into Vorbis Comments for comparison
    let mut vorbis_comments2 = split_remainder.merge_tag(split_tag);
    // Soft before comparison -> unordered comparison
    vorbis_comments1
        .items
        .sort_by(|(lhs_key, lhs_val), (rhs_key, rhs_val)| {
            lhs_key.cmp(rhs_key).then_with(|| lhs_val.cmp(rhs_val))
        });
    vorbis_comments2
        .items
        .sort_by(|(lhs_key, lhs_val), (rhs_key, rhs_val)| {
            lhs_key.cmp(rhs_key).then_with(|| lhs_val.cmp(rhs_val))
        });
    assert_eq!(vorbis_comments1.items, vorbis_comments2.items);
}

#[test]
fn zero_sized_vorbis_comments() {
    let tag_bytes = std::fs::read("tests/tags/assets/zero.vorbis").unwrap();
    let _ = read_tag(&tag_bytes);
}

#[test]
fn issue_60() {
    let tag_bytes = std::fs::read("tests/tags/assets/issue_60.vorbis").unwrap();
    let tag = read_tag(&tag_bytes);

    assert_eq!(tag.pictures().len(), 1);
    assert!(tag.items.is_empty());
}

#[test]
fn initial_key_roundtrip() {
    // Both the primary and alternate key should be mapped to the primary
    // key if stored again. Note: The outcome is undefined if both the
    // primary and alternate key would be stored redundantly in VorbisComments!
    for key in ["INITIALKEY", "KEY"] {
        let mut vorbis_comments = VorbisComments {
            items: vec![(key.to_owned(), "Cmaj".to_owned())],
            ..Default::default()
        };
        let mut tag = Tag::from(vorbis_comments);
        assert_eq!(Some("Cmaj"), tag.get_string(&ItemKey::InitialKey));
        tag.insert_text(ItemKey::InitialKey, "Cmin".to_owned());
        vorbis_comments = tag.into();
        assert_eq!(Some("Cmin"), vorbis_comments.get("INITIALKEY"));
    }
}
