use super::*;

#[test]
fn data_type_kind_matches_plain_app_ordinals() {
    assert_eq!(DataType::Default.kind(), 0);
    assert_eq!(DataType::Audio.kind(), 1);
    assert_eq!(DataType::Video.kind(), 2);
    assert_eq!(DataType::Image.kind(), 3);
    assert_eq!(DataType::Sms.kind(), 4);
    assert_eq!(DataType::Contact.kind(), 5);
    assert_eq!(DataType::Note.kind(), 6);
    assert_eq!(DataType::FeedEntry.kind(), 7);
    assert_eq!(DataType::Call.kind(), 8);
    assert_eq!(DataType::Package.kind(), 21);
    assert_eq!(DataType::File.kind(), 22);
    assert_eq!(DataType::AppFile.kind(), 23);
    assert_eq!(DataType::Doc.kind(), 24);
}

#[test]
fn data_type_storage_kinds_decode_without_changing_existing_values() {
    for (kind, expected) in [
        (0, DataType::Default), (1, DataType::Audio), (2, DataType::Video),
        (3, DataType::Image), (4, DataType::Sms), (5, DataType::Contact),
        (6, DataType::Note), (7, DataType::FeedEntry), (8, DataType::Call),
        (21, DataType::Package), (22, DataType::File), (23, DataType::AppFile),
        (24, DataType::Doc),
    ] {
        assert_eq!(DataType::from_kind(kind), Some(expected));
    }
    assert_eq!(DataType::from_kind(9), None);
}

#[test]
fn data_type_media_type_str_covers_media_domains_only() {
    assert_eq!(DataType::Audio.media_type_str(), Some("audio"));
    assert_eq!(DataType::Video.media_type_str(), Some("video"));
    assert_eq!(DataType::Image.media_type_str(), Some("image"));
    assert_eq!(DataType::Doc.media_type_str(), Some("doc"));
    assert_eq!(DataType::File.media_type_str(), None);
    assert_eq!(DataType::Sms.media_type_str(), None);
}
