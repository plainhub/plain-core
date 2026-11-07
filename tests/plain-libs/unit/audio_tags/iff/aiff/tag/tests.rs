use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::iff::aiff::{AiffTextChunks, Comment};
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::{ItemValue, Tag, TagItem, TagType};

use std::io::Cursor;

#[test]
fn parse_aiff_text() {
    let expected_tag = AiffTextChunks {
        name: Some(String::from("Foo title")),
        author: Some(String::from("Bar artist")),
        copyright: Some(String::from("Baz copyright")),
        annotations: Some(vec![
            String::from("Qux annotation"),
            String::from("Quux annotation"),
        ]),
        comments: Some(vec![
            Comment {
                timestamp: 1024,
                marker_id: 0,
                text: String::from("Quuz comment"),
            },
            Comment {
                timestamp: 2048,
                marker_id: 40,
                text: String::from("Corge comment"),
            },
        ]),
    };

    let tag = crate::audio_tags::tag::utils::read_path("tests/tags/assets/test.aiff_text");

    let parsed_tag = super::super::read::read_from(
        &mut Cursor::new(tag),
        ParseOptions::new().read_properties(false),
    )
    .unwrap()
    .text_chunks_tag
    .unwrap();

    assert_eq!(expected_tag, parsed_tag);
}

#[test]
fn aiff_text_to_tag() {
    let tag_bytes = crate::audio_tags::tag::utils::read_path("tests/tags/assets/test.aiff_text");

    let aiff_text = super::super::read::read_from(
        &mut Cursor::new(tag_bytes),
        ParseOptions::new().read_properties(false),
    )
    .unwrap()
    .text_chunks_tag
    .unwrap();

    let tag: Tag = aiff_text.into();

    assert_eq!(tag.get_string(&ItemKey::TrackTitle), Some("Foo title"));
    assert_eq!(tag.get_string(&ItemKey::TrackArtist), Some("Bar artist"));
    assert_eq!(
        tag.get_string(&ItemKey::CopyrightMessage),
        Some("Baz copyright")
    );

    let mut comments = tag.get_strings(&ItemKey::Comment);
    assert_eq!(comments.next(), Some("Qux annotation"));
    assert_eq!(comments.next(), Some("Quux annotation"));
    assert_eq!(comments.next(), Some("Quuz comment"));
    assert_eq!(comments.next(), Some("Corge comment"));
    assert!(comments.next().is_none());
}

#[test]
fn tag_to_aiff_text() {
    let mut tag = Tag::new(TagType::AiffText);
    tag.insert_text(ItemKey::TrackTitle, String::from("Foo title"));
    tag.insert_text(ItemKey::TrackArtist, String::from("Bar artist"));
    tag.insert_text(ItemKey::CopyrightMessage, String::from("Baz copyright"));
    tag.push_unchecked(TagItem::new(
        ItemKey::Comment,
        ItemValue::Text(String::from("Qux annotation")),
    ));
    tag.push_unchecked(TagItem::new(
        ItemKey::Comment,
        ItemValue::Text(String::from("Quux annotation")),
    ));

    let aiff_text: AiffTextChunks = tag.into();

    assert_eq!(aiff_text.name, Some(String::from("Foo title")));
    assert_eq!(aiff_text.author, Some(String::from("Bar artist")));
    assert_eq!(aiff_text.copyright, Some(String::from("Baz copyright")));
    assert_eq!(
        aiff_text.annotations,
        Some(vec![
            String::from("Qux annotation"),
            String::from("Quux annotation")
        ])
    );
    assert!(aiff_text.comments.is_none());
}

#[test]
fn zero_sized_text_chunks() {
    let tag_bytes = crate::audio_tags::tag::utils::read_path("tests/tags/assets/zero.aiff_text");

    let aiff_file = super::super::read::read_from(
        &mut Cursor::new(tag_bytes),
        ParseOptions::new().read_properties(false),
    )
    .unwrap();

    let aiff_text = aiff_file.text_chunks().unwrap();

    assert_eq!(aiff_text.name, Some(String::new()));
    assert_eq!(aiff_text.author, Some(String::new()));
    assert_eq!(aiff_text.annotations, Some(vec![String::new()]));
    assert_eq!(aiff_text.comments, None); // Comments have additional information we need, so we ignore on empty
    assert_eq!(aiff_text.copyright, Some(String::new()));
}
