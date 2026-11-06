use super::*;

#[test]
fn test_parse_shared_strings() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="5" uniqueCount="3">
    <si><t>Hello</t></si>
    <si><t>World</t></si>
    <si><t>Test</t></si>
</sst>"#;

    let ss = SharedStrings::parse(xml).unwrap();
    assert_eq!(ss.len(), 3);
    assert_eq!(ss.get(0), Some("Hello"));
    assert_eq!(ss.get(1), Some("World"));
    assert_eq!(ss.get(2), Some("Test"));
    assert_eq!(ss.get(3), None);
}

#[test]
fn test_rich_text() {
    // Rich text with runs - note: t element must include any trailing spaces
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <si>
        <r><t>Hello</t></r>
        <r><t>World</t></r>
    </si>
</sst>"#;

    let ss = SharedStrings::parse(xml).unwrap();
    assert_eq!(ss.len(), 1);
    // Rich text runs are concatenated as-is
    assert_eq!(ss.get(0), Some("HelloWorld"));
}

#[test]
fn test_malformed_shared_string_preserves_raw_entity_text() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <si><t>Hello &bogus; World</t></si>
</sst>"#;

    let ss = SharedStrings::parse(xml).unwrap();
    assert_eq!(ss.get(0), Some("Hello &bogus; World"));
}

#[test]
fn test_shared_strings_mixed_entities_preserve_legitimate_and_malformed() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1" uniqueCount="1">
  <si><t>A &amp; B &bogus; C</t></si>
</sst>"#;

    let table = SharedStrings::parse(xml).expect("parse succeeds");
    let s = table.get(0).expect("index 0 exists");
    assert_eq!(s, "A & B &bogus; C");
}

#[test]
fn test_shared_strings_cr_character_refs_collapse_to_lf() {
    // Excel stores in-cell line breaks as &#13;&#10; (CRLF) or bare &#13;.
    // Under quick-xml 0.40+ these arrive as separate GeneralRef events, so
    // the CRLF pair must be collapsed at the flush point, not per-event.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <si><t>line1&#13;&#10;line2&#13;line3&#xD;line4</t></si>
</sst>"#;

    let ss = SharedStrings::parse(xml).expect("parse succeeds");
    assert_eq!(ss.get(0), Some("line1\nline2\nline3\nline4"));
}

#[test]
fn test_shared_strings_numeric_and_predefined_refs_round_trip() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
    <si><t>&#48;&#x30; &lt;a&gt; &amp; &quot;q&quot; &apos;p&apos;</t></si>
</sst>"#;

    let ss = SharedStrings::parse(xml).expect("parse succeeds");
    assert_eq!(ss.get(0), Some("00 <a> & \"q\" 'p'"));
}
