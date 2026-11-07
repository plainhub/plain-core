use crate::audio_tags::tag::{ItemKey, Tag, TagType};
use std::fs::File;
use std::io::Read;

pub(crate) fn create_tag(tag_type: TagType) -> Tag {
    let mut tag = Tag::new(tag_type);

    tag.insert_text(ItemKey::TrackTitle, String::from("Foo title"));
    tag.insert_text(ItemKey::TrackArtist, String::from("Bar artist"));
    tag.insert_text(ItemKey::AlbumTitle, String::from("Baz album"));
    tag.insert_text(ItemKey::Comment, String::from("Qux comment"));
    tag.insert_text(ItemKey::TrackNumber, String::from("1"));
    tag.insert_text(ItemKey::Genre, String::from("Classical"));

    tag
}

pub(crate) fn verify_tag(tag: &Tag, track_number: bool, genre: bool) {
    assert_eq!(tag.get_string(&ItemKey::TrackTitle), Some("Foo title"));
    assert_eq!(tag.get_string(&ItemKey::TrackArtist), Some("Bar artist"));
    assert_eq!(tag.get_string(&ItemKey::AlbumTitle), Some("Baz album"));
    assert_eq!(tag.get_string(&ItemKey::Comment), Some("Qux comment"));

    if track_number {
        assert_eq!(tag.get_string(&ItemKey::TrackNumber), Some("1"));
    }

    if genre {
        assert_eq!(tag.get_string(&ItemKey::Genre), Some("Classical"));
    }
}

pub(crate) fn read_path(path: &str) -> Vec<u8> {
    read_file(&mut File::open(path).unwrap())
}

pub(crate) fn read_file(file: &mut File) -> Vec<u8> {
    let mut tag = Vec::new();

    file.read_to_end(&mut tag).unwrap();

    tag
}
