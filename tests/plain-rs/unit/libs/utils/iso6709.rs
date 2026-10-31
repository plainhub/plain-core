use super::*;

#[test]
fn signed_decimal_coordinates_preserve_precision_and_allow_optional_altitude() {
    for (raw, expected) in [
        ("+12.345678-123.456789/", (12.345678, -123.456789)),
        ("-90+180+3.25/", (-90.0, 180.0)),
        ("+0-0/", (0.0, -0.0)),
        ("+1.5-2.75", (1.5, -2.75)),
    ] {
        assert_eq!(coordinates(raw), Some(expected), "{raw}");
    }
}

#[test]
fn malformed_or_out_of_range_coordinates_are_not_partially_matched() {
    for raw in [
        "",
        "12.3456-123.1234/",
        "+91-3/",
        "+1+181/",
        "+1.2.3-2/",
        "+1-2+3+4/",
        "+1-2//",
        "junk+1.2345-2.3456/",
        "+1-2garbage/",
        "+.5-2/",
        "+NaN-2/",
        "+1-2e3/",
    ] {
        assert!(coordinates(raw).is_none(), "{raw}");
    }
}
