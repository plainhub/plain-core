use super::*;

#[test]
fn test_parse_numbering() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:abstractNum w:abstractNumId="0">
        <w:lvl w:ilvl="0">
            <w:start w:val="1"/>
            <w:numFmt w:val="decimal"/>
            <w:lvlText w:val="%1."/>
        </w:lvl>
        <w:lvl w:ilvl="1">
            <w:start w:val="1"/>
            <w:numFmt w:val="bullet"/>
            <w:lvlText w:val="•"/>
        </w:lvl>
    </w:abstractNum>
    <w:num w:numId="1">
        <w:abstractNumId w:val="0"/>
    </w:num>
</w:numbering>"#;

    let mut map = NumberingMap::parse(xml).unwrap();

    assert!(map.abstract_nums.contains_key("0"));
    assert!(map.instances.contains_key("1"));

    let abstract_num = map.abstract_nums.get("0").unwrap();
    assert_eq!(abstract_num.levels.len(), 2);
    assert_eq!(abstract_num.levels[0].num_fmt, "decimal");

    // Test list info
    let (list_type, num) = map.get_list_info("1", 0).unwrap();
    assert_eq!(list_type, ListType::Numbered);
    assert_eq!(num, 1);

    // Second call increments
    let (_, num) = map.get_list_info("1", 0).unwrap();
    assert_eq!(num, 2);
}

#[test]
fn test_list_numbering_reset_on_parent_change() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:abstractNum w:abstractNumId="0">
        <w:lvl w:ilvl="0">
            <w:start w:val="1"/>
            <w:numFmt w:val="decimal"/>
            <w:lvlText w:val="%1."/>
        </w:lvl>
        <w:lvl w:ilvl="1">
            <w:start w:val="1"/>
            <w:numFmt w:val="decimal"/>
            <w:lvlText w:val="%1.%2"/>
        </w:lvl>
    </w:abstractNum>
    <w:num w:numId="1">
        <w:abstractNumId w:val="0"/>
    </w:num>
</w:numbering>"#;

    let mut map = NumberingMap::parse(xml).unwrap();

    // 1. Level 0: 1
    let (_, num) = map.get_list_info("1", 0).unwrap();
    assert_eq!(num, 1);

    // 1.1 Level 1: 1
    let (_, num) = map.get_list_info("1", 1).unwrap();
    assert_eq!(num, 1);

    // 1.2 Level 1: 2
    let (_, num) = map.get_list_info("1", 1).unwrap();
    assert_eq!(num, 2);

    // 2. Level 0: 2 (moving back to parent should reset child level)
    let (_, num) = map.get_list_info("1", 0).unwrap();
    assert_eq!(num, 2);

    // 2.1 Level 1: should be reset to 1 (not 3)
    let (_, num) = map.get_list_info("1", 1).unwrap();
    assert_eq!(num, 1, "Child level should reset when parent changes");
}
