use super::parse_id3v2;
use crate::audio_tags::config::ParseOptions;

#[test]
fn zero_size_id3v2() {
    use crate::audio_tags::config::ParsingMode;
    use crate::audio_tags::id3::v2::header::Id3v2Header;

    use std::io::Cursor;

    let mut f = Cursor::new(crate::audio_tags::tag::utils::read_tag_fixture("id3v2/zero.id3v2"));
    let header = Id3v2Header::parse(&mut f).unwrap();
    assert!(
        parse_id3v2(
            &mut f,
            header,
            ParseOptions::new().parsing_mode(ParsingMode::Strict)
        )
        .is_ok()
    );
}

#[test]
fn bad_frame_id_relaxed_id3v2() {
    use crate::audio_tags::config::ParsingMode;
    use crate::audio_tags::id3::v2::header::Id3v2Header;
    use crate::audio_tags::prelude::*;

    use std::io::Cursor;

    // Contains a frame with a "+" in the ID, which is invalid.
    // All other frames in the tag are valid, however.
    let mut f = Cursor::new(
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/bad_frame_otherwise_valid.id3v24"),
    );
    let header = Id3v2Header::parse(&mut f).unwrap();
    let id3v2 = parse_id3v2(
        &mut f,
        header,
        ParseOptions::new().parsing_mode(ParsingMode::Relaxed),
    );
    assert!(id3v2.is_ok());

    let id3v2 = id3v2.unwrap();

    // There are 6 valid frames and 1 invalid frame
    assert_eq!(id3v2.len(), 6);

    assert_eq!(id3v2.title().as_deref(), Some("Foo title"));
    assert_eq!(id3v2.artist().as_deref(), Some("Bar artist"));
    assert_eq!(id3v2.comment().as_deref(), Some("Qux comment"));
    assert_eq!(id3v2.year(), Some(1984));
    assert_eq!(id3v2.track(), Some(1));
    assert_eq!(id3v2.genre().as_deref(), Some("Classical"));
}
