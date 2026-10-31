use super::*;

#[test]
fn dates_supported_by_the_removed_app_parser_remain_supported_in_shared_rust() {
    for (value, expected) in [
        ("Tue, 10 Sep 2024 12:00:00 GMT", 1725969600000),
        ("Tue, 10 Sep 2024 12:00 GMT", 1725969600000),
        ("10 Sep 24 12:00:00 GMT", 1725969600000),
        ("Wed, 02 Oct 2024 15:30:00 EST", 1727901000000),
        ("Tue, 10 Sep 2024 14:00:00 +0200", 1725969600000),
        ("2024-09-10T12:00:00Z", 1725969600000),
        ("2024-09-10T14:00:00+02:00", 1725969600000),
        ("2024-09-10T12:00:00.500Z", 1725969600500),
        ("2024-09-10", 1725926400000),
        ("2024-09-10 12:00:00", 1725969600000),
        ("2024-09-10 14:00:00 +0200", 1725969600000),
    ] {
        assert_eq!(
            parse(value).unwrap().timestamp_millis(),
            expected,
            "{value}"
        );
    }
    for value in ["not a date", "", "2024-02-30", "2024-09-10 25:00:00 +0200"] {
        assert!(parse(value).is_none(), "{value}");
    }
}
