use crate::audio_tags::id3::v2::{FrameFlags, FrameHeader, FrameId};

use std::borrow::Cow;

#[test]
fn issue_204_invalid_ufid_parsing_mode_best_attempt() {
    use crate::audio_tags::config::ParsingMode;
    use crate::audio_tags::id3::v2::UniqueFileIdentifierFrame;

    let ufid_no_owner = UniqueFileIdentifierFrame {
        header: FrameHeader::new(FrameId::Valid(Cow::Borrowed("UFID")), FrameFlags::default()),
        owner: String::new(),
        identifier: vec![0],
    };

    let bytes = ufid_no_owner.as_bytes();

    assert!(
        UniqueFileIdentifierFrame::parse(
            &mut &bytes[..],
            FrameFlags::default(),
            ParsingMode::Strict
        )
        .is_err()
    );
    assert!(
        UniqueFileIdentifierFrame::parse(
            &mut &bytes[..],
            FrameFlags::default(),
            ParsingMode::BestAttempt
        )
        .is_ok()
    );
}
