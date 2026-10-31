use super::*;
#[test]
fn framework_name_tables_are_unique_sorted_and_cover_the_old_packaged_assets() {
    for table in [ATTRIBUTES, STYLES] {
        assert!(table.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
    assert_eq!(attribute(0x01010001), Some("label"));
    assert_eq!(attribute(0x01010000), Some("theme"));
    assert_eq!(style(0x01030000), Some("Animation"));
    assert_eq!(attribute(0x7f010001), None);
    assert_eq!(style(0x7f010001), None);
}
