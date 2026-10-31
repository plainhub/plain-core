use super::*;
use chrono::{FixedOffset, LocalResult, Utc};

fn utc(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .timestamp_millis()
}

#[test]
fn numeric_inputs_preserve_the_seconds_and_milliseconds_boundary() {
    for (value, expected) in [
        ("0", 0),
        (" 1700000000 ", 1700000000000),
        ("999999999999", 999999999999000),
        ("1000000000000", 1000000000000),
        ("0000000000001", 1),
        ("9223372036854775807", i64::MAX),
    ] {
        assert_eq!(parse_in(value, &Utc), Some(expected), "{value}");
    }
    assert_eq!(parse_in("9223372036854775808", &Utc), None);
}

#[test]
fn local_dates_and_minutes_or_seconds_use_the_supplied_zone() {
    let zone = FixedOffset::east_opt(8 * 3600).unwrap();
    for (value, expected) in [
        ("2026-10-05", "2026-10-04T16:00:00Z"),
        ("2026-10-05T12:34", "2026-10-05T04:34:00Z"),
        (" 2026-10-05T12:34:56 ", "2026-10-05T04:34:56Z"),
        ("2024-02-29", "2024-02-28T16:00:00Z"),
    ] {
        assert_eq!(parse_in(value, &zone), Some(utc(expected)), "{value}");
    }
}

#[test]
fn invalid_and_previously_unsupported_shapes_remain_unparseable() {
    for value in [
        "",
        " ",
        "-1",
        "+1",
        "1.5",
        "2026-1-05",
        "2026-10-5",
        "2026-02-29",
        "2026-13-01",
        "2026-10-05T24:00",
        "2026-10-05T12:60",
        "2026-10-05T12:34:60",
        "2026-10-05 12:34",
        "2026-10-05T12:34:56Z",
        "2026-10-05T12:34:56+08:00",
        "2026-10-05T12:34:56.000",
        "2026-10-05T12:34junk",
        "١٢٣",
    ] {
        assert_eq!(parse_in(value, &Utc), None, "{value}");
    }
}

#[derive(Clone)]
struct Transition {
    at: NaiveDateTime,
    before: FixedOffset,
    after: FixedOffset,
}

impl Transition {
    fn new(at: &str, before: i32, after: i32) -> Self {
        Self {
            at: DateTime::parse_from_rfc3339(at).unwrap().naive_utc(),
            before: FixedOffset::east_opt(before).unwrap(),
            after: FixedOffset::east_opt(after).unwrap(),
        }
    }
}

impl TimeZone for Transition {
    type Offset = FixedOffset;

    fn from_offset(offset: &FixedOffset) -> Self {
        Self {
            at: NaiveDateTime::MIN,
            before: *offset,
            after: *offset,
        }
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<FixedOffset> {
        self.offset_from_local_datetime(&local.and_hms_opt(0, 0, 0).unwrap())
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
        let before = *local - TimeDelta::seconds(i64::from(self.before.local_minus_utc()));
        let after = *local - TimeDelta::seconds(i64::from(self.after.local_minus_utc()));
        match (before < self.at, after >= self.at) {
            (true, true) => LocalResult::Ambiguous(self.before, self.after),
            (true, false) => LocalResult::Single(self.before),
            (false, true) => LocalResult::Single(self.after),
            (false, false) => LocalResult::None,
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&utc.and_hms_opt(0, 0, 0).unwrap())
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        if *utc < self.at {
            self.before
        } else {
            self.after
        }
    }
}

#[test]
fn local_datetimes_shift_forward_in_gaps_and_choose_the_earlier_overlap() {
    let spring = Transition::new("2026-03-29T01:00:00Z", 3600, 7200);
    assert_eq!(
        parse_in("2026-03-29T02:30", &spring),
        Some(utc("2026-03-29T01:30:00Z"))
    );
    assert_eq!(
        parse_in("2026-03-29T03:00", &spring),
        Some(utc("2026-03-29T01:00:00Z"))
    );
    let autumn = Transition::new("2026-10-25T01:00:00Z", 7200, 3600);
    assert_eq!(
        parse_in("2026-10-25T02:30", &autumn),
        Some(utc("2026-10-25T00:30:00Z"))
    );
}

#[test]
fn dates_start_at_the_first_valid_instant_even_when_midnight_is_skipped() {
    let midnight_gap = Transition::new("2026-10-04T23:30:00Z", 0, 3600);
    assert_eq!(
        parse_in("2026-10-05", &midnight_gap),
        Some(utc("2026-10-04T23:30:00Z"))
    );
    assert_eq!(
        parse_in("2026-10-05T00:00", &midnight_gap),
        Some(utc("2026-10-05T00:00:00Z"))
    );
    let skipped_day = Transition::new("2011-12-30T10:00:00Z", -36000, 50400);
    assert_eq!(
        parse_in("2011-12-30", &skipped_day),
        Some(utc("2011-12-30T10:00:00Z"))
    );
    assert_eq!(
        parse_in("2011-12-30T12:00", &skipped_day),
        Some(utc("2011-12-30T22:00:00Z"))
    );
}
