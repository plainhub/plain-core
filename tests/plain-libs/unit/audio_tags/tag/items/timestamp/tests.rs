use crate::audio_tags::config::ParsingMode;
use crate::audio_tags::tag::items::timestamp::Timestamp;

fn expected() -> Timestamp {
    // 2024-06-03T14:08:49
    Timestamp {
        year: 2024,
        month: Some(6),
        day: Some(3),
        hour: Some(14),
        minute: Some(8),
        second: Some(49),
    }
}

#[test]
fn timestamp_decode() {
    let content = "2024-06-03T14:08:49";
    let parsed_timestamp = Timestamp::parse(&mut content.as_bytes(), ParsingMode::Strict).unwrap();

    assert_eq!(parsed_timestamp, Some(expected()));
}

#[test]
fn timestamp_decode_no_zero() {
    // Zeroes are not used
    let content = "2024-6-3T14:8:49";

    let parsed_timestamp =
        Timestamp::parse(&mut content.as_bytes(), ParsingMode::BestAttempt).unwrap();

    assert_eq!(parsed_timestamp, Some(expected()));
}

#[test]
fn timestamp_decode_zero_substitution() {
    // Zeros are replaced by spaces
    let content = "2024- 6- 3T14: 8:49";

    let parsed_timestamp =
        Timestamp::parse(&mut content.as_bytes(), ParsingMode::BestAttempt).unwrap();

    assert_eq!(parsed_timestamp, Some(expected()));
}

#[test]
fn timestamp_encode() {
    let encoded = expected().to_string();
    assert_eq!(encoded, "2024-06-03T14:08:49");
}

#[test]
fn timestamp_encode_invalid() {
    let mut timestamp = expected();

    // Hour, minute, and second have a dependency on day
    timestamp.day = None;
    assert_eq!(timestamp.to_string().len(), 7);
}

fn broken_timestamps() -> [(&'static [u8], Timestamp); 7] {
    [
        (
            b"2024-",
            Timestamp {
                year: 2024,
                ..Timestamp::default()
            },
        ),
        (
            b"2024-06-",
            Timestamp {
                year: 2024,
                month: Some(6),
                ..Timestamp::default()
            },
        ),
        (
            b"2024--",
            Timestamp {
                year: 2024,
                ..Timestamp::default()
            },
        ),
        (
            b"2024-  -",
            Timestamp {
                year: 2024,
                ..Timestamp::default()
            },
        ),
        (
            b"2024-06-03T",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                ..Timestamp::default()
            },
        ),
        (
            b"2024:06",
            Timestamp {
                year: 2024,
                ..Timestamp::default()
            },
        ),
        (
            b"2024-0-",
            Timestamp {
                year: 2024,
                month: Some(0),
                ..Timestamp::default()
            },
        ),
    ]
}

#[test]
fn reject_broken_timestamps_strict() {
    for (timestamp, _) in broken_timestamps() {
        let parsed_timestamp = Timestamp::parse(&mut &timestamp[..], ParsingMode::Strict);
        assert!(parsed_timestamp.is_err());
    }
}

#[test]
fn accept_broken_timestamps_best_attempt() {
    for (timestamp, partial_result) in broken_timestamps() {
        let parsed_timestamp = Timestamp::parse(&mut &timestamp[..], ParsingMode::BestAttempt);
        assert!(parsed_timestamp.is_ok());
        assert_eq!(
            parsed_timestamp.unwrap(),
            Some(partial_result),
            "{}",
            timestamp.escape_ascii()
        );
    }
}

#[test]
fn timestamp_decode_partial() {
    let partial_timestamps: [(&[u8], Timestamp); 6] = [
        (
            b"2024",
            Timestamp {
                year: 2024,
                ..Timestamp::default()
            },
        ),
        (
            b"2024-06",
            Timestamp {
                year: 2024,
                month: Some(6),
                ..Timestamp::default()
            },
        ),
        (
            b"2024-06-03",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                ..Timestamp::default()
            },
        ),
        (
            b"2024-06-03T14",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                hour: Some(14),
                ..Timestamp::default()
            },
        ),
        (
            b"2024-06-03T14:08",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                hour: Some(14),
                minute: Some(8),
                ..Timestamp::default()
            },
        ),
        (b"2024-06-03T14:08:49", expected()),
    ];

    for (data, expected) in partial_timestamps {
        let parsed_timestamp = Timestamp::parse(&mut &data[..], ParsingMode::Strict).unwrap();
        assert_eq!(parsed_timestamp, Some(expected));
    }
}

#[test]
fn empty_timestamp() {
    let empty_timestamp = Timestamp::parse(&mut "".as_bytes(), ParsingMode::BestAttempt).unwrap();
    assert!(empty_timestamp.is_none());

    let empty_timestamp_strict = Timestamp::parse(&mut "".as_bytes(), ParsingMode::Strict);
    assert!(empty_timestamp_strict.is_err());
}

#[test]
fn timestamp_no_separators() {
    let timestamp = "20240603T140849";
    let parsed_timestamp =
        Timestamp::parse(&mut timestamp.as_bytes(), ParsingMode::BestAttempt).unwrap();
    assert_eq!(parsed_timestamp, Some(expected()));
}

#[test]
fn timestamp_decode_partial_no_separators() {
    let partial_timestamps: [(&[u8], Timestamp); 6] = [
        (
            b"2024",
            Timestamp {
                year: 2024,
                ..Timestamp::default()
            },
        ),
        (
            b"202406",
            Timestamp {
                year: 2024,
                month: Some(6),
                ..Timestamp::default()
            },
        ),
        (
            b"20240603",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                ..Timestamp::default()
            },
        ),
        (
            b"20240603T14",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                hour: Some(14),
                ..Timestamp::default()
            },
        ),
        (
            b"20240603T1408",
            Timestamp {
                year: 2024,
                month: Some(6),
                day: Some(3),
                hour: Some(14),
                minute: Some(8),
                ..Timestamp::default()
            },
        ),
        (b"20240603T140849", expected()),
    ];

    for (data, expected) in partial_timestamps {
        let parsed_timestamp = Timestamp::parse(&mut &data[..], ParsingMode::Strict)
            .unwrap_or_else(|e| panic!("{e}: {}", std::str::from_utf8(data).unwrap()));
        assert_eq!(parsed_timestamp, Some(expected));
    }
}

#[test]
fn timestamp_no_time_marker() {
    let timestamp = "2024-06-03 14:08:49";

    let parsed_timestamp_strict = Timestamp::parse(&mut timestamp.as_bytes(), ParsingMode::Strict);
    assert!(parsed_timestamp_strict.is_err());

    let parsed_timestamp_best_attempt =
        Timestamp::parse(&mut timestamp.as_bytes(), ParsingMode::BestAttempt).unwrap();
    assert_eq!(
        parsed_timestamp_best_attempt,
        Some(Timestamp {
            year: 2024,
            month: Some(6),
            day: Some(3),
            ..Timestamp::default()
        })
    );
}
