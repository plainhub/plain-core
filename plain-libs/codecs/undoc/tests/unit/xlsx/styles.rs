use super::*;

#[test]
fn test_builtin_date_formats() {
    let styles = Styles::default();

    // Built-in date formats (14-22)
    assert!(styles.is_date_format(14)); // m/d/yyyy
    assert!(styles.is_date_format(15)); // d-mmm-yy
    assert!(styles.is_date_format(16)); // d-mmm
    assert!(styles.is_date_format(17)); // mmm-yy
    assert!(styles.is_date_format(22)); // m/d/yy h:mm

    // Not date formats
    assert!(!styles.is_date_format(0)); // General
    assert!(!styles.is_date_format(1)); // 0
    assert!(!styles.is_date_format(2)); // 0.00
}

#[test]
fn test_custom_date_format_detection() {
    assert!(Styles::is_date_format_code("mmmm\\ d\\,\\ yyyy"));
    assert!(Styles::is_date_format_code("yyyy-mm-dd"));
    assert!(Styles::is_date_format_code("d/m/yy"));
    assert!(Styles::is_date_format_code("[$-409]mmmm\\ d\\,\\ yyyy;@"));

    // Not date formats
    assert!(!Styles::is_date_format_code("0.00"));
    assert!(!Styles::is_date_format_code("#,##0"));
    assert!(!Styles::is_date_format_code("\"$\"#,##0.00"));
}

#[test]
fn test_serial_to_date() {
    // Excel serial dates
    assert_eq!(Styles::serial_to_date(1.0), Some("1900-01-01".to_string()));
    assert_eq!(Styles::serial_to_date(2.0), Some("1900-01-02".to_string()));
    assert_eq!(Styles::serial_to_date(59.0), Some("1900-02-28".to_string()));
    // Note: serial 60 is the fake Feb 29, 1900
    assert_eq!(Styles::serial_to_date(61.0), Some("1900-03-01".to_string()));

    // More recent dates
    assert_eq!(
        Styles::serial_to_date(44197.0),
        Some("2021-01-01".to_string())
    );
    assert_eq!(
        Styles::serial_to_date(45658.0),
        Some("2025-01-01".to_string())
    );

    // With time component
    assert_eq!(
        Styles::serial_to_date(44197.5),
        Some("2021-01-01T12:00:00".to_string())
    );
}
