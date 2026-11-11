use super::*;
#[test]
fn all_http_date_forms_preserve_the_same_instant() {
    let canonical = "Sun, 06 Nov 1994 08:49:37 GMT";
    let expected = std::time::UNIX_EPOCH + std::time::Duration::from_secs(784111777);
    for text in [
        canonical,
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
    ] {
        assert_eq!(parse_http_date(text).unwrap(), expected);
    }
    assert_eq!(fmt_http_date(expected), canonical);
    for text in [
        "Sun, 31 Feb 1994 08:49:37 GMT",
        "Sun, 06 Nov 1994 25:49:37 GMT",
        "invalid",
    ] {
        assert!(parse_http_date(text).is_err());
    }
}
