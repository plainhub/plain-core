use super::*;

#[test]
fn test_parse_styles() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:style w:type="paragraph" w:styleId="Heading1">
        <w:name w:val="Heading 1"/>
        <w:basedOn w:val="Normal"/>
        <w:pPr>
            <w:outlineLvl w:val="0"/>
        </w:pPr>
        <w:rPr>
            <w:b/>
            <w:sz w:val="32"/>
        </w:rPr>
    </w:style>
</w:styles>"#;

    let map = StyleMap::parse(xml).unwrap();
    assert!(map.styles.contains_key("Heading1"));

    let style = map.styles.get("Heading1").unwrap();
    assert_eq!(style.name, "Heading 1");
    assert_eq!(style.outline_level, Some(0));
    assert_eq!(style.run_props.bold, Some(true));
    assert_eq!(style.run_props.font_size, Some(32));
}

#[test]
fn test_heading_level() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
    <w:style w:type="paragraph" w:styleId="Title">
        <w:name w:val="Title"/>
    </w:style>
    <w:style w:type="paragraph" w:styleId="Heading2">
        <w:name w:val="Heading 2"/>
        <w:pPr>
            <w:outlineLvl w:val="1"/>
        </w:pPr>
    </w:style>
</w:styles>"#;

    let map = StyleMap::parse(xml).unwrap();
    assert_eq!(map.get_heading_level("Title"), HeadingLevel::H1);
    assert_eq!(map.get_heading_level("Heading2"), HeadingLevel::H2);
    assert_eq!(map.get_heading_level("Unknown"), HeadingLevel::None);
}
