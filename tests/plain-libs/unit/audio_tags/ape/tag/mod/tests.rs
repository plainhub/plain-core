use crate::audio_tags::ape::{ApeItem, ApeTag};
use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::id3::v2::util::pairs::DEFAULT_NUMBER_IN_PAIR;
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::{ItemValue, Tag, TagItem, TagType};

use crate::audio_tags::picture::{MimeType, Picture, PictureType};
use std::io::Cursor;

#[test]
fn parse_ape() {
    let mut expected_tag = ApeTag::default();

    let title_item = ApeItem::new(
        String::from("TITLE"),
        ItemValue::Text(String::from("Foo title")),
    )
    .unwrap();

    let artist_item = ApeItem::new(
        String::from("ARTIST"),
        ItemValue::Text(String::from("Bar artist")),
    )
    .unwrap();

    let album_item = ApeItem::new(
        String::from("ALBUM"),
        ItemValue::Text(String::from("Baz album")),
    )
    .unwrap();

    let comment_item = ApeItem::new(
        String::from("COMMENT"),
        ItemValue::Text(String::from("Qux comment")),
    )
    .unwrap();

    let year_item =
        ApeItem::new(String::from("YEAR"), ItemValue::Text(String::from("1984"))).unwrap();

    let track_number_item =
        ApeItem::new(String::from("TRACK"), ItemValue::Text(String::from("1"))).unwrap();

    let genre_item = ApeItem::new(
        String::from("GENRE"),
        ItemValue::Text(String::from("Classical")),
    )
    .unwrap();

    expected_tag.insert(title_item);
    expected_tag.insert(artist_item);
    expected_tag.insert(album_item);
    expected_tag.insert(comment_item);
    expected_tag.insert(year_item);
    expected_tag.insert(track_number_item);
    expected_tag.insert(genre_item);

    let tag = crate::audio_tags::tag::utils::read_tag_fixture("test.apev2");
    let mut reader = Cursor::new(tag);

    let (Some(parsed_tag), _) =
        crate::audio_tags::ape::tag::read::read_ape_tag(&mut reader, false, ParseOptions::new())
            .unwrap()
    else {
        unreachable!();
    };

    assert_eq!(expected_tag.len(), parsed_tag.len());

    for item in &expected_tag.items {
        assert!(parsed_tag.items.contains(item));
    }
}

#[test]
fn ape_to_tag() {
    let tag_bytes = crate::audio_tags::tag::utils::read_tag_fixture("test.apev2");
    let mut reader = Cursor::new(tag_bytes);

    let (Some(ape), _) =
        crate::audio_tags::ape::tag::read::read_ape_tag(&mut reader, false, ParseOptions::new())
            .unwrap()
    else {
        unreachable!();
    };

    let tag: Tag = ape.into();

    crate::audio_tags::tag::utils::verify_tag(&tag, true, true);
}

#[test]
fn tag_to_ape() {
    fn verify_key(tag: &ApeTag, key: &str, expected_val: &str) {
        assert_eq!(
            tag.get(key).map(ApeItem::value),
            Some(&ItemValue::Text(String::from(expected_val)))
        );
    }

    let tag = crate::audio_tags::tag::utils::create_tag(TagType::Ape);

    let ape_tag: ApeTag = tag.into();

    verify_key(&ape_tag, "Title", "Foo title");
    verify_key(&ape_tag, "Artist", "Bar artist");
    verify_key(&ape_tag, "Album", "Baz album");
    verify_key(&ape_tag, "Comment", "Qux comment");
    verify_key(&ape_tag, "Track", "1");
    verify_key(&ape_tag, "Genre", "Classical");
}

#[test]
fn set_track() {
    let mut ape = ApeTag::default();
    let track = 1;

    ape.set_track(track);

    assert_eq!(ape.track().unwrap(), track);
    assert!(ape.track_total().is_none());
}

#[test]
fn set_track_total() {
    let mut ape = ApeTag::default();
    let track_total = 2;

    ape.set_track_total(track_total);

    assert_eq!(ape.track().unwrap(), DEFAULT_NUMBER_IN_PAIR);
    assert_eq!(ape.track_total().unwrap(), track_total);
}

#[test]
fn set_track_and_track_total() {
    let mut ape = ApeTag::default();
    let track = 1;
    let track_total = 2;

    ape.set_track(track);
    ape.set_track_total(track_total);

    assert_eq!(ape.track().unwrap(), track);
    assert_eq!(ape.track_total().unwrap(), track_total);
}

#[test]
fn set_track_total_and_track() {
    let mut ape = ApeTag::default();
    let track_total = 2;
    let track = 1;

    ape.set_track_total(track_total);
    ape.set_track(track);

    assert_eq!(ape.track_total().unwrap(), track_total);
    assert_eq!(ape.track().unwrap(), track);
}

#[test]
fn set_disk() {
    let mut ape = ApeTag::default();
    let disk = 1;

    ape.set_disk(disk);

    assert_eq!(ape.disk().unwrap(), disk);
    assert!(ape.disk_total().is_none());
}

#[test]
fn set_disk_total() {
    let mut ape = ApeTag::default();
    let disk_total = 2;

    ape.set_disk_total(disk_total);

    assert_eq!(ape.disk().unwrap(), DEFAULT_NUMBER_IN_PAIR);
    assert_eq!(ape.disk_total().unwrap(), disk_total);
}

#[test]
fn set_disk_and_disk_total() {
    let mut ape = ApeTag::default();
    let disk = 1;
    let disk_total = 2;

    ape.set_disk(disk);
    ape.set_disk_total(disk_total);

    assert_eq!(ape.disk().unwrap(), disk);
    assert_eq!(ape.disk_total().unwrap(), disk_total);
}

#[test]
fn set_disk_total_and_disk() {
    let mut ape = ApeTag::default();
    let disk_total = 2;
    let disk = 1;

    ape.set_disk_total(disk_total);
    ape.set_disk(disk);

    assert_eq!(ape.disk_total().unwrap(), disk_total);
    assert_eq!(ape.disk().unwrap(), disk);
}

#[test]
fn track_number_tag_to_ape() {
    let track_number = 1;

    let mut tag = Tag::new(TagType::Ape);

    tag.push(TagItem::new(
        ItemKey::TrackNumber,
        ItemValue::Text(track_number.to_string()),
    ));

    let tag: ApeTag = tag.into();

    assert_eq!(tag.track().unwrap(), track_number);
    assert!(tag.track_total().is_none());
}

#[test]
fn track_total_tag_to_ape() {
    let track_total = 2;

    let mut tag = Tag::new(TagType::Ape);

    tag.push(TagItem::new(
        ItemKey::TrackTotal,
        ItemValue::Text(track_total.to_string()),
    ));

    let tag: ApeTag = tag.into();

    assert_eq!(tag.track().unwrap(), DEFAULT_NUMBER_IN_PAIR);
    assert_eq!(tag.track_total().unwrap(), track_total);
}

#[test]
fn track_number_and_track_total_tag_to_ape() {
    let track_number = 1;
    let track_total = 2;

    let mut tag = Tag::new(TagType::Ape);

    tag.push(TagItem::new(
        ItemKey::TrackNumber,
        ItemValue::Text(track_number.to_string()),
    ));

    tag.push(TagItem::new(
        ItemKey::TrackTotal,
        ItemValue::Text(track_total.to_string()),
    ));

    let tag: ApeTag = tag.into();

    assert_eq!(tag.track().unwrap(), track_number);
    assert_eq!(tag.track_total().unwrap(), track_total);
}

#[test]
fn disk_number_tag_to_ape() {
    let disk_number = 1;

    let mut tag = Tag::new(TagType::Ape);

    tag.push(TagItem::new(
        ItemKey::DiscNumber,
        ItemValue::Text(disk_number.to_string()),
    ));

    let tag: ApeTag = tag.into();

    assert_eq!(tag.disk().unwrap(), disk_number);
    assert!(tag.disk_total().is_none());
}

#[test]
fn disk_total_tag_to_ape() {
    let disk_total = 2;

    let mut tag = Tag::new(TagType::Ape);

    tag.push(TagItem::new(
        ItemKey::DiscTotal,
        ItemValue::Text(disk_total.to_string()),
    ));

    let tag: ApeTag = tag.into();

    assert_eq!(tag.disk().unwrap(), DEFAULT_NUMBER_IN_PAIR);
    assert_eq!(tag.disk_total().unwrap(), disk_total);
}

#[test]
fn disk_number_and_disk_total_tag_to_ape() {
    let disk_number = 1;
    let disk_total = 2;

    let mut tag = Tag::new(TagType::Ape);

    tag.push(TagItem::new(
        ItemKey::DiscNumber,
        ItemValue::Text(disk_number.to_string()),
    ));

    tag.push(TagItem::new(
        ItemKey::DiscTotal,
        ItemValue::Text(disk_total.to_string()),
    ));

    let tag: ApeTag = tag.into();

    assert_eq!(tag.disk().unwrap(), disk_number);
    assert_eq!(tag.disk_total().unwrap(), disk_total);
}
