use super::*;

#[test]
fn hyperlink_targets_are_read_from_the_instruction() {
    assert_eq!(
        hyperlink_target(" HYPERLINK \"https://example.com/a b\" "),
        Some(LinkTarget::Address("https://example.com/a b".into()))
    );
    assert_eq!(
        hyperlink_target("HYPERLINK \\l \"_Toc1\""),
        Some(LinkTarget::Bookmark("_Toc1".into()))
    );
    assert_eq!(
        hyperlink_target("HYPERLINK https://example.com"),
        Some(LinkTarget::Address("https://example.com".into()))
    );
    assert_eq!(hyperlink_target(" PAGEREF _Toc1 \\h "), None);
}
