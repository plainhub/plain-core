use super::*;

#[test]
fn test_default_english_mappings() {
    let mapping = StyleMapping::with_defaults();

    assert_eq!(mapping.get_by_name("Heading 1"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_name("heading 1"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_name("HEADING 1"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_name("Title"), Some(HeadingLevel::H1));
}

#[test]
fn test_default_korean_mappings() {
    let mapping = StyleMapping::with_defaults();

    assert_eq!(mapping.get_by_name("제목 1"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_name("제목1"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_name("제목 2"), Some(HeadingLevel::H2));
}

#[test]
fn test_id_mappings() {
    let mapping = StyleMapping::with_defaults();

    assert_eq!(mapping.get_by_id("Heading1"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_id("Title"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_id("Unknown"), None);
}

#[test]
fn test_combined_lookup() {
    let mapping = StyleMapping::with_defaults();

    // Name takes precedence
    assert_eq!(
        mapping.get(Some("Heading1"), Some("제목 1")),
        Some(HeadingLevel::H1)
    );

    // Fall back to ID
    assert_eq!(mapping.get(Some("Heading2"), None), Some(HeadingLevel::H2));

    // Neither matches
    assert_eq!(mapping.get(Some("Unknown"), Some("Unknown")), None);
}

#[test]
fn test_custom_mapping() {
    let mut mapping = StyleMapping::new();
    mapping.add_name_mapping("Custom Title", HeadingLevel::H1);
    mapping.add_id_mapping("CustomID", HeadingLevel::H3);

    assert_eq!(mapping.get_by_name("custom title"), Some(HeadingLevel::H1));
    assert_eq!(mapping.get_by_id("CustomID"), Some(HeadingLevel::H3));
}
