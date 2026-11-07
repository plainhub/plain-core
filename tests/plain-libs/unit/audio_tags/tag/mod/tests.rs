use super::try_parse_year;
use crate::audio_tags::picture::{Picture, PictureType};
use crate::audio_tags::prelude::*;
use crate::audio_tags::tag::utils::read_path;
use crate::audio_tags::tag::{Tag, TagType};

use std::io::{Seek, Write};
use std::process::Command;

#[test]
fn should_preserve_empty_title() {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(String::from("Foo title"));

    assert_eq!(tag.title().as_deref(), Some("Foo title"));

    tag.set_title(String::new());
    assert_eq!(tag.title().as_deref(), Some(""));

    tag.remove_title();
    assert_eq!(tag.title(), None);
}

#[test]
fn try_parse_year_with_leading_trailing_whitespace_and_various_formats() {
    assert_eq!(Some(1983), try_parse_year("\t 1983\n"));
    assert_eq!(Some(1983), try_parse_year("1983-1"));
    assert_eq!(Some(1983), try_parse_year("1983- 1"));
    assert_eq!(Some(1983), try_parse_year("1983-01"));
    assert_eq!(Some(1983), try_parse_year("1983-1-2"));
    assert_eq!(Some(1983), try_parse_year("1983- 1- 2"));
    assert_eq!(Some(1983), try_parse_year("1983-01-02T10:24:08Z"));
    assert_eq!(Some(1983), try_parse_year("1983-01-02T10:24:08.001Z"));
}

#[test]
fn should_not_parse_year_from_less_than_4_digits() {
    assert!(try_parse_year("198").is_none());
    assert!(try_parse_year("19").is_none());
    assert!(try_parse_year("1").is_none());
}
