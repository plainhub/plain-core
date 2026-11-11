use super::*;
#[test]
fn timestamps_offsets_nanoseconds_and_serde_keep_their_meaning() {
    let value = DateTime::parse_from_rfc3339("2000-02-29T23:59:59.123456789+08:00").unwrap();
    assert_eq!(
        value.with_timezone(&Utc).to_rfc3339(),
        "2000-02-29T15:59:59.123456789+00:00"
    );
    let json = crate::serde_json::to_string(&value).unwrap();
    assert_eq!(
        crate::serde_json::from_str::<DateTime<FixedOffset>>(&json).unwrap(),
        value
    );
    let before = DateTime::<Utc>::from_timestamp(-1, 999_999_999).unwrap();
    assert_eq!(before.timestamp_millis(), -1);
    assert_eq!(before.timestamp_nanos_opt(), Some(-1));
    assert_eq!(DateTime::<Utc>::from(std::time::UNIX_EPOCH).timestamp(), 0);
}
#[test]
fn calendar_and_parse_boundaries_remain_checked() {
    assert!(NaiveDate::from_ymd_opt(2000, 2, 29).is_some());
    assert!(NaiveDate::from_ymd_opt(1900, 2, 29).is_none());
    for input in [
        "2024-02-30T00:00:00Z",
        "2024-01-01T25:00:00Z",
        "2024-01-01T00:00:00+25:00",
        "bad",
    ] {
        assert!(DateTime::parse_from_rfc3339(input).is_err(), "{input}");
    }
    assert!(DateTime::<Utc>::from_timestamp(i64::MAX, 0).is_none());
    assert!(NaiveDate::MAX.checked_add_days(Days::new(1)).is_none());
    assert!(NaiveDate::MIN.checked_sub_days(Days::new(1)).is_none());
}
