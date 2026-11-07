use crate::audio_tags::config::{ParseOptions, ParsingMode};
use crate::audio_tags::mp4::ilst::TITLE;
use crate::audio_tags::mp4::ilst::atom::AtomDataStorage;
use crate::audio_tags::mp4::read::AtomReader;
use crate::audio_tags::mp4::{AdvisoryRating, Atom, AtomData, AtomIdent, DataType, Ilst, Mp4File};
use crate::audio_tags::picture::{MimeType, Picture, PictureType};
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::utils;
use crate::audio_tags::tag::utils::read_path;
use crate::audio_tags::tag::{ItemValue, Tag, TagItem, TagType};

use std::io::{Cursor, Read as _, Seek as _, Write as _};

fn read_ilst(path: &str, parse_options: ParseOptions) -> Ilst {
    let tag = std::fs::read(path).unwrap();
    read_ilst_raw(&tag, parse_options)
}

fn read_ilst_raw(bytes: &[u8], parse_options: ParseOptions) -> Ilst {
    read_ilst_with_options(bytes, parse_options)
}

fn read_ilst_strict(path: &str) -> Ilst {
    read_ilst(path, ParseOptions::new().parsing_mode(ParsingMode::Strict))
}

fn read_ilst_bestattempt(path: &str) -> Ilst {
    read_ilst(
        path,
        ParseOptions::new().parsing_mode(ParsingMode::BestAttempt),
    )
}

fn read_ilst_with_options(bytes: &[u8], parse_options: ParseOptions) -> Ilst {
    let len = bytes.len();

    let cursor = Cursor::new(bytes);
    let mut reader = AtomReader::new(cursor, parse_options.parsing_mode).unwrap();

    super::read::parse_ilst(&mut reader, parse_options, len as u64).unwrap()
}

fn verify_atom(ilst: &Ilst, ident: [u8; 4], data: &AtomData) {
    let atom = ilst.get(&AtomIdent::Fourcc(ident)).unwrap();
    assert_eq!(atom.data().next().unwrap(), data);
}

#[test]
fn parse_ilst() {
    let mut expected_tag = Ilst::default();

    // The track number is stored with a code 0,
    // meaning the there is no need to indicate the type,
    // which is `u64` in this case
    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"trkn"),
        AtomData::Unknown {
            code: DataType::Reserved,
            data: vec![0, 0, 0, 1, 0, 0, 0, 0],
        },
    ));

    // Same with disc numbers
    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"disk"),
        AtomData::Unknown {
            code: DataType::Reserved,
            data: vec![0, 0, 0, 1, 0, 2],
        },
    ));

    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"\xa9ART"),
        AtomData::UTF8(String::from("Bar artist")),
    ));

    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"\xa9alb"),
        AtomData::UTF8(String::from("Baz album")),
    ));

    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"\xa9cmt"),
        AtomData::UTF8(String::from("Qux comment")),
    ));

    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"\xa9day"),
        AtomData::UTF8(String::from("1984")),
    ));

    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"\xa9gen"),
        AtomData::UTF8(String::from("Classical")),
    ));

    expected_tag.insert(Atom::new(
        AtomIdent::Fourcc(*b"\xa9nam"),
        AtomData::UTF8(String::from("Foo title")),
    ));

    let tag = crate::audio_tags::tag::utils::read_path("tests/tags/assets/ilst/test.ilst");
    let len = tag.len();

    let cursor = Cursor::new(tag);
    let mut reader = AtomReader::new(cursor, ParsingMode::Strict).unwrap();

    let parsed_tag = super::read::parse_ilst(
        &mut reader,
        ParseOptions::new().parsing_mode(ParsingMode::Strict),
        len as u64,
    )
    .unwrap();

    assert_eq!(expected_tag, parsed_tag);
}

#[test]
fn ilst_to_tag() {
    let tag = crate::audio_tags::tag::utils::read_path("tests/tags/assets/ilst/test.ilst");
    let len = tag.len();

    let cursor = Cursor::new(tag);
    let mut reader = AtomReader::new(cursor, ParsingMode::Strict).unwrap();

    let ilst = super::read::parse_ilst(
        &mut reader,
        ParseOptions::new().parsing_mode(ParsingMode::Strict),
        len as u64,
    )
    .unwrap();

    let tag: Tag = ilst.into();

    crate::audio_tags::tag::utils::verify_tag(&tag, true, true);

    assert_eq!(tag.get_string(&ItemKey::DiscNumber), Some("1"));
    assert_eq!(tag.get_string(&ItemKey::DiscTotal), Some("2"));
}

#[test]
fn tag_to_ilst() {
    let mut tag = crate::audio_tags::tag::utils::create_tag(TagType::Mp4Ilst);

    tag.insert_text(ItemKey::DiscNumber, String::from("1"));
    tag.insert_text(ItemKey::DiscTotal, String::from("2"));

    let ilst: Ilst = tag.into();

    verify_atom(
        &ilst,
        *b"\xa9nam",
        &AtomData::UTF8(String::from("Foo title")),
    );
    verify_atom(
        &ilst,
        *b"\xa9ART",
        &AtomData::UTF8(String::from("Bar artist")),
    );
    verify_atom(
        &ilst,
        *b"\xa9alb",
        &AtomData::UTF8(String::from("Baz album")),
    );
    verify_atom(
        &ilst,
        *b"\xa9cmt",
        &AtomData::UTF8(String::from("Qux comment")),
    );
    verify_atom(
        &ilst,
        *b"\xa9gen",
        &AtomData::UTF8(String::from("Classical")),
    );
    verify_atom(
        &ilst,
        *b"trkn",
        &AtomData::Unknown {
            code: DataType::Reserved,
            data: vec![0, 0, 0, 1, 0, 0, 0, 0],
        },
    );
    verify_atom(
        &ilst,
        *b"disk",
        &AtomData::Unknown {
            code: DataType::Reserved,
            data: vec![0, 0, 0, 1, 0, 2, 0, 0],
        },
    )
}

#[test]
fn issue_34() {
    let ilst = read_ilst_strict("tests/tags/assets/ilst/issue_34.ilst");

    verify_atom(
        &ilst,
        *b"\xa9ART",
        &AtomData::UTF8(String::from("Foo artist")),
    );
    verify_atom(
        &ilst,
        *b"plID",
        &AtomData::Unknown {
            code: DataType::BeSignedInteger,
            data: 88888_u64.to_be_bytes().to_vec(),
        },
    )
}

#[test]
fn advisory_rating() {
    let ilst = read_ilst_strict("tests/tags/assets/ilst/advisory_rating.ilst");

    verify_atom(
        &ilst,
        *b"\xa9ART",
        &AtomData::UTF8(String::from("Foo artist")),
    );

    assert_eq!(ilst.advisory_rating(), Some(AdvisoryRating::Explicit));
}

#[test]
fn read_non_full_meta_atom() {
    let file_bytes = read_path(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/upstream/lofty/assets/non_full_meta_atom.m4a"
    ));
    let file = Mp4File::read_from(
        &mut Cursor::new(file_bytes),
        ParseOptions::new().read_properties(false),
    )
    .unwrap();

    assert!(file.ilst_tag.is_some());
}

#[test]
fn multi_value_atom() {
    let ilst = read_ilst_strict("tests/tags/assets/ilst/multi_value_atom.ilst");
    let artist_atom = ilst.get(&AtomIdent::Fourcc(*b"\xa9ART")).unwrap();

    assert_eq!(
        artist_atom.data,
        AtomDataStorage::Multiple(vec![
            AtomData::UTF8(String::from("Foo artist")),
            AtomData::UTF8(String::from("Bar artist")),
        ])
    );

    // Sanity single value atom
    verify_atom(
        &ilst,
        *b"\xa9gen",
        &AtomData::UTF8(String::from("Classical")),
    );
}

#[test]
fn multi_value_roundtrip() {
    let mut tag = Tag::new(TagType::Mp4Ilst);
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
    assert_eq!(20, tag.len());

    let ilst = Ilst::from(tag.clone());
    let (split_remainder, split_tag) = ilst.split_tag();

    assert_eq!(0, split_remainder.len());
    assert_eq!(tag.len(), split_tag.len());
    assert_eq!(tag.items, split_tag.items);
}

#[test]
fn zero_sized_ilst() {
    let file = Mp4File::read_from(
        &mut Cursor::new(read_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/upstream/lofty/assets/zero/zero.ilst"
        ))),
        ParseOptions::new().read_properties(false),
    )
    .unwrap();

    assert_eq!(file.ilst(), Some(&Ilst::default()));
}

#[test]
fn merge_insert() {
    let mut ilst = Ilst::new();

    // Insert two titles
    ilst.set_title(String::from("Foo"));
    ilst.insert(Atom::new(TITLE, AtomData::UTF8(String::from("Bar"))));

    // Title should still be the first value, but there should be two total values
    assert_eq!(ilst.title().as_deref(), Some("Foo"));
    assert_eq!(ilst.get(&TITLE).unwrap().data().count(), 2);

    // Meaning we only have 1 atom
    assert_eq!(ilst.len(), 1);
}

#[test]
fn invalid_atom_type() {
    let ilst = read_ilst_strict("tests/tags/assets/ilst/invalid_atom_type.ilst");

    // The tag contains 3 items, however the last one has an invalid type. We will stop at that point, but retain the
    // first two items.
    assert_eq!(ilst.len(), 2);

    assert_eq!(ilst.track().unwrap(), 1);
    assert_eq!(ilst.track_total().unwrap(), 0);
    assert_eq!(ilst.disk().unwrap(), 1);
    assert_eq!(ilst.disk_total().unwrap(), 2);
}

#[test]
fn invalid_string_encoding() {
    let ilst = read_ilst_bestattempt("tests/tags/assets/ilst/invalid_string_encoding.ilst");

    // The tag has an album string with some unknown encoding, but the rest of the tag
    // is valid. We should have all items present except the album.
    assert_eq!(ilst.len(), 3);

    assert_eq!(ilst.artist().unwrap(), "Foo artist");
    assert_eq!(ilst.title().unwrap(), "Bar title");
    assert_eq!(ilst.comment().unwrap(), "Baz comment");

    assert!(ilst.album().is_none());
}

#[test]
fn flag_item_conversion() {
    let mut tag = Tag::new(TagType::Mp4Ilst);
    tag.insert_text(ItemKey::FlagCompilation, "1".to_owned());
    tag.insert_text(ItemKey::FlagPodcast, "0".to_owned());

    let ilst: Ilst = tag.into();
    assert_eq!(
        ilst.get(&AtomIdent::Fourcc(*b"cpil"))
            .unwrap()
            .data()
            .next()
            .unwrap(),
        &AtomData::Bool(true)
    );
    assert_eq!(
        ilst.get(&AtomIdent::Fourcc(*b"pcst"))
            .unwrap()
            .data()
            .next()
            .unwrap(),
        &AtomData::Bool(false)
    );
}

#[test]
fn gnre_conversion_case_1() {
    // Case 1: 1 `gnre` atom present, no `©gen` present. `gnre` gets converted without issue.
    let ilst = read_ilst_bestattempt("tests/tags/assets/ilst/gnre_conversion_case_1.ilst");

    assert_eq!(ilst.len(), 2);
    assert_eq!(ilst.artist().unwrap(), "Foo artist"); // Sanity check
    assert_eq!(ilst.genre().unwrap(), "Funk");
}

#[test]
fn gnre_conversion_case_2() {
    // Case 2: 1 `gnre` atom present, 1 `©gen` present. `gnre` gets discarded.
    let ilst = read_ilst_bestattempt("tests/tags/assets/ilst/gnre_conversion_case_2.ilst");

    assert_eq!(ilst.len(), 2);
    assert_eq!(ilst.artist().unwrap(), "Foo artist"); // Sanity check
    assert_eq!(ilst.genre().unwrap(), "My Custom Genre");
}

#[test]
fn gnre_conversion_case_3() {
    // Case 2: 1 `gnre` atom present, 1 `©gen` present. implicit conversion are disabled, `gnre` is retained
    //         as an unknown atom.
    let ilst = read_ilst(
        "tests/tags/assets/ilst/gnre_conversion_case_2.ilst",
        ParseOptions::new().implicit_conversions(false),
    );

    assert_eq!(ilst.len(), 3);
    assert_eq!(ilst.artist().unwrap(), "Foo artist"); // Sanity check
    assert_eq!(ilst.genre().unwrap(), "My Custom Genre");
    assert_eq!(
        ilst.get(&AtomIdent::Fourcc(*b"gnre"))
            .unwrap()
            .data()
            .next()
            .unwrap(),
        &AtomData::Unknown {
            code: DataType::BeSignedInteger,
            data: vec![0, 6]
        }
    );
}
