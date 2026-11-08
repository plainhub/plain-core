use crate::audio_tags::id3::v2::{
    Event, EventTimingCodesFrame, EventType, FrameFlags, FrameHeader, FrameId, TimestampFormat,
};

fn expected() -> EventTimingCodesFrame<'static> {
    EventTimingCodesFrame {
        header: FrameHeader {
            id: FrameId::Valid(std::borrow::Cow::Borrowed("ETCO")),
            flags: FrameFlags::default(),
        },
        timestamp_format: TimestampFormat::MS,
        events: vec![
            Event {
                event_type: EventType::IntroStart,
                timestamp: 1500,
            },
            Event {
                event_type: EventType::IntroEnd,
                timestamp: 5000,
            },
            Event {
                event_type: EventType::MainPartStart,
                timestamp: 7500,
            },
            Event {
                event_type: EventType::MainPartEnd,
                timestamp: 900_000,
            },
            Event {
                event_type: EventType::AudioFileEnds,
                timestamp: 750_000_000,
            },
        ],
    }
}

#[test]
fn etco_decode() {
    let cont = crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.etco");

    let parsed_etco = EventTimingCodesFrame::parse(&mut &cont[..], FrameFlags::default())
        .unwrap()
        .unwrap();

    assert_eq!(parsed_etco, expected());
}

#[test]
fn etco_encode() {
    let encoded = expected().as_bytes();

    let expected_bytes =
        crate::audio_tags::tag::utils::read_tag_fixture("id3v2/test.etco");

    assert_eq!(encoded, expected_bytes);
}
