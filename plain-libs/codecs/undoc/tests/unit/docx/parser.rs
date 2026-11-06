use super::*;

/// A container with no parts, for tests that parse a fragment of document XML. It must
/// be a real (empty) ZIP archive: zero bytes are not one, and the tests that built their
/// container from them returned early, unchecked, every time.
fn empty_container() -> crate::container::OoxmlContainer {
    let bytes = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()))
        .finish()
        .expect("an empty archive")
        .into_inner();
    crate::container::OoxmlContainer::from_bytes(bytes).expect("an empty archive opens")
}

// =========================================================================
// Whitespace Preservation Tests (GitHub Issue #2)
// =========================================================================

#[test]
fn test_whitespace_preserved_between_runs() {
    // Test case from GitHub Issue #2: "DATE OF BIRTH" was becoming "DATEOFBIRTH"
    // When xml:space="preserve" is used, spaces must be preserved
    let xml = r#"<w:p>
            <w:r><w:t>DATE</w:t></w:r>
            <w:r><w:t xml:space="preserve"> </w:t></w:r>
            <w:r><w:t>OF</w:t></w:r>
            <w:r><w:t xml:space="preserve"> </w:t></w:r>
            <w:r><w:t>BIRTH</w:t></w:r>
        </w:p>"#;

    // Create a minimal container just for testing paragraph parsing
    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    // The text should have spaces preserved
    assert!(
        text.contains("DATE") && text.contains("OF") && text.contains("BIRTH"),
        "Expected 'DATE OF BIRTH' with spaces, got: '{}'",
        text
    );
    // Check that spaces are actually there
    assert!(
        text.contains(' '),
        "Expected spaces between words, got: '{}'",
        text
    );
}

#[test]
fn test_whitespace_leading_trailing_preserved() {
    // Test leading/trailing whitespace with xml:space="preserve"
    let xml = r#"<w:p>
            <w:r><w:t xml:space="preserve">  Hello World  </w:t></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    // Leading/trailing spaces should be preserved
    assert!(
        text.starts_with("  ") || text.contains("  Hello"),
        "Expected leading spaces, got: '{}'",
        text
    );
}

#[test]
fn test_tab_character_handling() {
    // Test <w:tab/> element is converted to tab character
    let xml = r#"<w:p>
            <w:r>
                <w:t>Column1</w:t>
            </w:r>
            <w:r>
                <w:tab/>
            </w:r>
            <w:r>
                <w:t>Column2</w:t>
            </w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    assert!(
        text.contains('\t'),
        "Expected tab character between columns, got: '{}'",
        text
    );
    assert!(
        text.contains("Column1") && text.contains("Column2"),
        "Expected both column texts, got: '{}'",
        text
    );
}

#[test]
fn test_multiple_spaces_preserved() {
    // Test multiple consecutive spaces are preserved
    let xml = r#"<w:p>
            <w:r><w:t xml:space="preserve">Word1     Word2</w:t></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    // Multiple spaces should be preserved
    assert!(
        text.contains("     "),
        "Expected 5 consecutive spaces, got: '{}'",
        text
    );
}

#[test]
fn test_carriage_return_handling() {
    // Test <w:cr/> element creates line break
    let xml = r#"<w:p>
            <w:r>
                <w:t>Line1</w:t>
                <w:cr/>
                <w:t>Line2</w:t>
            </w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();

    // Check that we have a line break somewhere
    let has_line_break = para.runs.iter().any(|r| r.line_break);
    assert!(has_line_break, "Expected line break from <w:cr/>");
}

#[test]
fn test_non_breaking_hyphen() {
    // Test <w:noBreakHyphen/> is converted to non-breaking hyphen Unicode
    let xml = r#"<w:p>
            <w:r>
                <w:t>non</w:t>
                <w:noBreakHyphen/>
                <w:t>breaking</w:t>
            </w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    // Should contain non-breaking hyphen (U+2011) or at least the text parts
    assert!(
        text.contains("non") && text.contains("breaking"),
        "Expected 'non' and 'breaking' text, got: '{}'",
        text
    );
    assert!(
        text.contains('\u{2011}'),
        "Expected non-breaking hyphen U+2011, got: '{}'",
        text
    );
}

// =========================================================================
// Tracked Changes Tests (Revisions)
// =========================================================================

#[test]
fn test_tracked_changes_insertion() {
    // Test <w:ins> element marks text as inserted
    let xml = r#"<w:p>
            <w:r><w:t>Original </w:t></w:r>
            <w:ins>
                <w:r><w:t>inserted </w:t></w:r>
            </w:ins>
            <w:r><w:t>text</w:t></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();

    // Check that we have an inserted revision
    let has_inserted = para
        .runs
        .iter()
        .any(|r| r.revision == RevisionType::Inserted);
    assert!(has_inserted, "Expected to find inserted revision");

    // The inserted text should be marked
    let inserted_text: String = para
        .runs
        .iter()
        .filter(|r| r.revision == RevisionType::Inserted)
        .map(|r| r.text.as_str())
        .collect();
    assert!(
        inserted_text.contains("inserted"),
        "Expected 'inserted' text in revision, got: '{}'",
        inserted_text
    );
}

#[test]
fn test_tracked_changes_deletion() {
    // Test <w:del> element marks text as deleted
    let xml = r#"<w:p>
            <w:r><w:t>Keep this </w:t></w:r>
            <w:del>
                <w:r><w:t>deleted </w:t></w:r>
            </w:del>
            <w:r><w:t>text</w:t></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();

    // Check that we have a deleted revision
    let has_deleted = para
        .runs
        .iter()
        .any(|r| r.revision == RevisionType::Deleted);
    assert!(has_deleted, "Expected to find deleted revision");

    // The deleted text should be marked
    let deleted_text: String = para
        .runs
        .iter()
        .filter(|r| r.revision == RevisionType::Deleted)
        .map(|r| r.text.as_str())
        .collect();
    assert!(
        deleted_text.contains("deleted"),
        "Expected 'deleted' text in revision, got: '{}'",
        deleted_text
    );
}

// =========================================================================
// Footnote / Endnote Tests
// =========================================================================

#[test]
fn test_parse_footnotes_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="0" w:type="separator">
                <w:p><w:r><w:t>___</w:t></w:r></w:p>
            </w:footnote>
            <w:footnote w:id="1" w:type="continuationSeparator">
                <w:p><w:r><w:t>---</w:t></w:r></w:p>
            </w:footnote>
            <w:footnote w:id="2">
                <w:p><w:r><w:t>This is footnote two.</w:t></w:r></w:p>
            </w:footnote>
            <w:footnote w:id="3">
                <w:p><w:r><w:t>Another footnote.</w:t></w:r></w:p>
            </w:footnote>
        </w:footnotes>"#;

    let notes = parse_notes_xml(xml, "w:footnote");

    // Separator and continuationSeparator should be skipped
    assert!(!notes.contains_key("0"), "separator should be skipped");
    assert!(
        !notes.contains_key("1"),
        "continuationSeparator should be skipped"
    );

    // Content footnotes should be parsed
    assert_eq!(notes.get("2").unwrap(), "This is footnote two.");
    assert_eq!(notes.get("3").unwrap(), "Another footnote.");
    assert_eq!(notes.len(), 2);
}

#[test]
fn test_parse_endnotes_xml() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:endnote w:id="0" w:type="separator">
                <w:p><w:r><w:t>___</w:t></w:r></w:p>
            </w:endnote>
            <w:endnote w:id="1">
                <w:p><w:r><w:t>Endnote content here.</w:t></w:r></w:p>
            </w:endnote>
        </w:endnotes>"#;

    let notes = parse_notes_xml(xml, "w:endnote");

    assert!(!notes.contains_key("0"), "separator should be skipped");
    assert_eq!(notes.get("1").unwrap(), "Endnote content here.");
    assert_eq!(notes.len(), 1);
}

#[test]
fn test_footnote_multi_run_text() {
    // Footnote with multiple runs should concatenate text
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="1">
                <w:p>
                    <w:r><w:t>First </w:t></w:r>
                    <w:r><w:t>second </w:t></w:r>
                    <w:r><w:t>third.</w:t></w:r>
                </w:p>
            </w:footnote>
        </w:footnotes>"#;

    let notes = parse_notes_xml(xml, "w:footnote");
    assert_eq!(notes.get("1").unwrap(), "First second third.");
}

#[test]
fn test_parse_notes_xml_preserves_raw_malformed_entity() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="2">
                <w:p><w:r><w:t>Footnote &bogus; text</w:t></w:r></w:p>
            </w:footnote>
        </w:footnotes>"#;

    let notes = parse_notes_xml(xml, "w:footnote");
    assert_eq!(notes.get("2").unwrap(), "Footnote &bogus; text");
}

#[test]
fn test_footnote_reference_in_paragraph() {
    // Test that w:footnoteReference inserts [^N] marker
    let xml = r#"<w:p>
            <w:r><w:t>Some text</w:t></w:r>
            <w:r><w:footnoteReference w:id="2"/></w:r>
            <w:r><w:t> more text</w:t></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut footnotes = HashMap::new();
    footnotes.insert("2".to_string(), "Footnote content".to_string());

    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes,
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    assert!(
        text.contains("[^2]"),
        "Expected footnote reference [^2], got: '{}'",
        text
    );
    assert!(
        text.contains("Some text"),
        "Expected original text, got: '{}'",
        text
    );
}

#[test]
fn test_endnote_reference_in_paragraph() {
    // Test that w:endnoteReference inserts [^eN] marker
    let xml = r#"<w:p>
            <w:r><w:t>Text with endnote</w:t></w:r>
            <w:r><w:endnoteReference w:id="1"/></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut endnotes = HashMap::new();
    endnotes.insert("1".to_string(), "Endnote content".to_string());

    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(),
        endnotes,
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    assert!(
        text.contains("[^e1]"),
        "Expected endnote reference [^e1], got: '{}'",
        text
    );
}

#[test]
fn test_footnote_reference_skipped_when_no_content() {
    // If the footnote id doesn't exist in the map, no marker should be inserted
    let xml = r#"<w:p>
            <w:r><w:t>Text</w:t></w:r>
            <w:r><w:footnoteReference w:id="99"/></w:r>
        </w:p>"#;

    let container = empty_container();
    let mut parser = DocxParser {
        container,
        styles: StyleMap::default(),
        numbering: NumberingMap::default(),
        relationships: crate::container::Relationships::default(),
        footnotes: HashMap::new(), // No footnotes
        endnotes: HashMap::new(),
    };

    let para = parser.parse_paragraph(xml).unwrap();
    let text = para.plain_text();

    assert!(
        !text.contains("[^"),
        "Should not insert marker for unknown footnote, got: '{}'",
        text
    );
}

#[test]
fn test_empty_footnote_skipped() {
    // Footnotes with only whitespace should not be included
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:footnote w:id="1">
                <w:p><w:r><w:t>   </w:t></w:r></w:p>
            </w:footnote>
            <w:footnote w:id="2">
                <w:p><w:r><w:t>Real content.</w:t></w:r></w:p>
            </w:footnote>
        </w:footnotes>"#;

    let notes = parse_notes_xml(xml, "w:footnote");
    assert!(
        !notes.contains_key("1"),
        "Whitespace-only note should be skipped"
    );
    assert_eq!(notes.get("2").unwrap(), "Real content.");
}

#[test]
fn test_parse_header_footer_xml_basic() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:p>
                <w:r><w:t>Company Name</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t>Confidential</w:t></w:r>
            </w:p>
        </w:hdr>"#;

    let paragraphs = empty_test_parser().parse_header_footer_xml(xml);
    assert_eq!(paragraphs.len(), 2);
    assert_eq!(paragraphs[0].plain_text(), "Company Name");
    assert_eq!(paragraphs[1].plain_text(), "Confidential");
}

#[test]
fn test_parse_header_footer_xml_empty_paragraphs_skipped() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:p></w:p>
            <w:p>
                <w:r><w:t>Page 1</w:t></w:r>
            </w:p>
            <w:p>
                <w:r><w:t>   </w:t></w:r>
            </w:p>
        </w:ftr>"#;

    let paragraphs = empty_test_parser().parse_header_footer_xml(xml);
    assert_eq!(paragraphs.len(), 1);
    assert_eq!(paragraphs[0].plain_text(), "Page 1");
}

#[test]
fn test_parse_header_footer_xml_multiple_runs() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:p>
                <w:r><w:t>Draft - </w:t></w:r>
                <w:r><w:t>Do Not Distribute</w:t></w:r>
            </w:p>
        </w:hdr>"#;

    let paragraphs = empty_test_parser().parse_header_footer_xml(xml);
    assert_eq!(paragraphs.len(), 1);
    assert_eq!(paragraphs[0].plain_text(), "Draft - Do Not Distribute");
}

#[test]
fn test_parse_header_footer_xml_preserves_raw_malformed_entity() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:p><w:r><w:t>Footer &bogus; text</w:t></w:r></w:p>
        </w:ftr>"#;

    let paragraphs = empty_test_parser().parse_header_footer_xml(xml);
    assert_eq!(paragraphs.len(), 1);
    assert_eq!(paragraphs[0].plain_text(), "Footer &bogus; text");
}

#[test]
fn test_parse_header_footer_xml_empty() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
        </w:hdr>"#;

    let paragraphs = empty_test_parser().parse_header_footer_xml(xml);
    assert!(paragraphs.is_empty());
}

// =========================================================================
// Text Box Content Extraction Tests (w:txbxContent)
// =========================================================================

/// A package whose main document part holds bytes that are not UTF-8. Construction
/// succeeds -- it does not read that part -- and `parse` fails, which is exactly the
/// path a lenient stream takes when the whole document cannot be parsed.
fn docx_with_unreadable_document() -> Vec<u8> {
    use std::io::{Cursor, Write};
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(b"<w:document>caf\xe9</w:document>").unwrap();
    zip.finish().unwrap().into_inner()
}

/// Counts the events a lenient stream delivers when every callback returns `flow`.
fn lenient_events_after_a_failed_parse(flow: std::ops::ControlFlow<()>) -> usize {
    let mut parser = DocxParser::from_bytes(docx_with_unreadable_document()).unwrap();
    let opts = crate::streaming::SectionStreamOptions {
        lenient: true,
        ..Default::default()
    };
    let mut events = 0;
    parser
        .for_each_section(opts, |_| {
            events += 1;
            flow
        })
        .unwrap();
    events
}

/// Control: a consumer that keeps going sees start, the failure, and the end -- so the
/// failure path is really the one being exercised below.
#[test]
fn test_lenient_stream_reports_a_failed_parse_as_three_events() {
    assert_eq!(
        lenient_events_after_a_failed_parse(std::ops::ControlFlow::Continue(())),
        3
    );
}

/// `ControlFlow::Break` stops the stream -- the streaming API's documented contract,
/// which the PPTX and XLSX streams keep on every event. This path used to ignore it.
#[test]
fn test_lenient_stream_stops_when_the_consumer_breaks_after_a_failed_parse() {
    assert_eq!(
        lenient_events_after_a_failed_parse(std::ops::ControlFlow::Break(())),
        1,
        "Break on DocumentStart must end the stream"
    );
}

const EMPTY_DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;

/// A body paragraph whose style has outline level 0 becomes a level-1 heading; the
/// paragraph after it, with no style, stays body text.
#[test]
fn test_heading_style_makes_a_body_paragraph_a_heading() {
    use crate::model::HeadingLevel;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Introduction</w:t></w:r></w:p>
    <w:p><w:r><w:t>Body text</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
    let styles = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
</w:styles>"#;
    let data = create_docx_with_parts(
        document_xml,
        EMPTY_DOCUMENT_RELS,
        &[("word/styles.xml", styles)],
    );
    let doc = DocxParser::from_bytes(data).unwrap().parse().unwrap();

    let paras: Vec<&crate::model::Paragraph> = doc.sections[0]
        .content
        .iter()
        .filter_map(|block| match block {
            crate::model::Block::Paragraph(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(paras[0].plain_text(), "Introduction");
    assert_eq!(paras[0].heading, HeadingLevel::H1);
    assert_eq!(paras[1].plain_text(), "Body text");
    assert!(!paras[1].heading.is_heading());
}

/// An internal image relationship whose media part exists becomes one image resource,
/// keyed by the relationship id and carrying the part's bytes.
#[test]
fn test_image_relationship_becomes_a_resource() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:r><w:drawing><a:blip r:embed="rIdImg"/></w:drawing></w:r></w:p></w:body>
</w:document>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#;
    let data = create_docx_with_parts(
        document_xml,
        rels,
        &[("word/media/image1.png", "stand-in image bytes")],
    );
    let doc = DocxParser::from_bytes(data).unwrap().parse().unwrap();

    assert_eq!(doc.resources.len(), 1);
    let resource = &doc.resources["rIdImg"];
    assert!(resource.is_image());
    assert_eq!(resource.filename.as_deref(), Some("image1.png"));
    assert_eq!(resource.data, b"stand-in image bytes");
}

/// A picture whose `a:blip` carries an extension list (an SVG original, an HD Photo
/// layer) arrives as a start tag, not an empty one. It must still be an image — in a
/// body paragraph and in a table cell, which are read by separate paths — and its
/// companions must be marked as belonging to it.
#[test]
fn test_blip_with_extension_list_is_an_image_and_its_companions_are_marked() {
    let picture = r#"<w:r><w:drawing><wp:inline><wp:docPr id="1" name="Picture 1" descr="Site plan"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="rIdPng"><a:extLst><a:ext uri="{BEBA8EAE-BF5A-486C-A8C5-ECC9F3942E4B}"><a14:imgProps><a14:imgLayer r:embed="rIdWdp"/></a14:imgProps></a:ext><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip r:embed="rIdSvg"/></a:ext></a:extLst></a:blip></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#;
    let document_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
            xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main"
            xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>{picture}</w:p>
    <w:tbl><w:tr><w:tc><w:p>{picture}</w:p></w:tc></w:tr></w:tbl>
  </w:body>
</w:document>"#
    );
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdPng" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdSvg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.svg"/>
  <Relationship Id="rIdWdp" Type="http://schemas.microsoft.com/office/2007/relationships/hdphoto" Target="media/hdphoto1.wdp"/>
</Relationships>"#;
    let data = create_docx_with_parts(
        &document_xml,
        rels,
        &[
            ("word/media/image1.png", "png"),
            ("word/media/image2.svg", "<svg/>"),
            ("word/media/hdphoto1.wdp", "wdp"),
        ],
    );
    let doc = DocxParser::from_bytes(data).unwrap().parse().unwrap();

    let body_images: Vec<_> = doc.sections[0]
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p.images.iter()),
            _ => None,
        })
        .flatten()
        .map(|i| (i.resource_id.as_str(), i.alt_text.as_deref()))
        .collect();
    assert_eq!(body_images, [("rIdPng", Some("Site plan"))]);

    let cell_images: Vec<_> = doc.sections[0]
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .flat_map(|t| t.rows.iter().flat_map(|r| r.cells.iter()))
        .flat_map(|c| c.content.iter().flat_map(|p| p.images.iter()))
        .map(|i| i.resource_id.as_str())
        .collect();
    assert_eq!(cell_images, ["rIdPng"]);

    assert_eq!(doc.resources.len(), 3);
    let png = &doc.resources["rIdPng"];
    assert_eq!(png.role, crate::model::ResourceRole::Primary);
    assert_eq!(png.alt_text.as_deref(), Some("Site plan"));
    let svg = &doc.resources["rIdSvg"];
    assert_eq!(svg.role, crate::model::ResourceRole::Alternate);
    assert_eq!(svg.companion_of.as_deref(), Some("rIdPng"));
    let wdp = &doc.resources["rIdWdp"];
    assert_eq!(wdp.role, crate::model::ResourceRole::Layer);
    assert_eq!(wdp.companion_of.as_deref(), Some("rIdPng"));

    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(
        md.contains("Site plan"),
        "the picture must reach Markdown:\n{md}"
    );
}

/// Helper to create a minimal DOCX in memory with given document.xml content.
fn create_minimal_docx(document_xml: &str) -> Vec<u8> {
    create_minimal_docx_with_document_rels(
        document_xml,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
        ),
    )
}

fn create_minimal_docx_with_document_rels(
    document_xml: &str,
    document_rels_xml: Option<&str>,
) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

    // _rels/.rels
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

    if let Some(document_rels_xml) = document_rels_xml {
        // word/_rels/document.xml.rels
        zip.start_file("word/_rels/document.xml.rels", options)
            .unwrap();
        zip.write_all(document_rels_xml.as_bytes()).unwrap();
    }

    // word/document.xml
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

/// Build a minimal DOCX with a single extra optional part containing
/// CP-1252 bytes (e.g. raw `Café`), which is valid XML syntactically but
/// not valid UTF-8 / UTF-16. Used to verify that `read_xml_optional`
/// surfaces `Error::Encoding` rather than silently treating the part as
/// absent.
fn create_minimal_docx_with_malformed_optional_part(extra_part_path: &str) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p/></w:body>
</w:document>"#,
    )
    .unwrap();

    zip.start_file(extra_part_path, options).unwrap();
    zip.write_all(b"<?xml version=\"1.0\"?><root>Caf\xe9</root>")
        .unwrap();

    zip.finish().unwrap().into_inner()
}

#[test]
fn test_docx_non_utf8_optional_parts_surface_encoding_error() {
    // Optional DOCX parts with malformed (non-UTF-8/UTF-16) byte content
    // must surface Error::Encoding instead of being silently dropped as
    // if the part were absent.
    for part_path in &[
        "word/styles.xml",
        "word/numbering.xml",
        "word/footnotes.xml",
        "word/endnotes.xml",
    ] {
        let data = create_minimal_docx_with_malformed_optional_part(part_path);
        let err = match DocxParser::from_bytes(data) {
            Ok(_) => panic!("malformed {part_path} must surface Error::Encoding"),
            Err(err) => err,
        };
        assert!(
            matches!(err, Error::Encoding(_)),
            "expected Error::Encoding for {part_path}, got {err:?}"
        );
    }
}

fn empty_test_parser() -> DocxParser {
    DocxParser::from_bytes(create_minimal_docx(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p/></w:body>
</w:document>"#,
    ))
    .unwrap()
}

#[test]
fn test_textbox_content_extracted() {
    // Text box via w:drawing > wps:txbx > w:txbxContent
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
  <w:body>
    <w:p>
      <w:r>
        <w:t>Normal paragraph</w:t>
      </w:r>
    </w:p>
    <w:p>
      <w:r>
        <w:drawing>
          <wps:wsp>
            <wps:txbx>
              <w:txbxContent>
                <w:p>
                  <w:r><w:t>Text box content here</w:t></w:r>
                </w:p>
              </w:txbxContent>
            </wps:txbx>
          </wps:wsp>
        </w:drawing>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = doc.plain_text();

    assert!(
        text.contains("Normal paragraph"),
        "Should contain normal paragraph text"
    );
    assert!(
        text.contains("Text box content here"),
        "Should contain text box content, got: {}",
        text
    );
}

// A table nested inside a text box (which lives inside a paragraph) must not
// be treated as a body-level table. Its text is extracted via the text-box
// path; entering table mode for it left an empty table shell (all cells
// blank, since the cell text was captured separately) as a spurious block.
#[test]
fn test_textbox_with_table_does_not_emit_empty_body_table() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
  <w:body>
    <w:p>
      <w:r>
        <w:drawing>
          <wps:wsp>
            <wps:txbx>
              <w:txbxContent>
                <w:tbl>
                  <w:tr><w:tc><w:p><w:r><w:t>TextboxTableCell</w:t></w:r></w:p></w:tc></w:tr>
                </w:tbl>
              </w:txbxContent>
            </wps:txbx>
          </wps:wsp>
        </w:drawing>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    assert!(
        doc.plain_text().contains("TextboxTableCell"),
        "text box table text lost, got: {:?}",
        doc.plain_text()
    );
    let table_blocks = doc.sections[0]
        .content
        .iter()
        .filter(|b| matches!(b, crate::model::Block::Table(_)))
        .count();
    assert_eq!(
        table_blocks, 0,
        "text-box table must not surface as a body table block"
    );
}

#[test]
fn test_docx_allows_missing_document_relationships_when_unused() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body>
</w:document>"#;

    let data = create_minimal_docx_with_document_rels(doc_xml, None);
    let mut parser =
        DocxParser::from_bytes(data).expect("missing document relationships should be optional");
    let doc = parser.parse().unwrap();

    assert_eq!(doc.plain_text(), "Hello");
}

#[test]
fn test_docx_rejects_malformed_document_relationships() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body>
</w:document>"#;

    let data = create_minimal_docx_with_document_rels(doc_xml, Some("<Relationships"));
    let err = DocxParser::from_bytes(data)
        .err()
        .expect("malformed document relationships should fail");

    match err {
        Error::XmlParseWithContext { location, .. } => {
            assert_eq!(location, "word/_rels/document.xml.rels")
        }
        other => panic!("expected malformed document rels error, got {other:?}"),
    }
}

#[test]
fn test_docx_body_malformed_entity_preserves_raw_text() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Hello &bogus; body</w:t></w:r></w:p></w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    assert_eq!(doc.plain_text(), "Hello &bogus; body");
}

#[test]
fn test_docx_textbox_malformed_entity_preserves_raw_text() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
  <w:body>
    <w:p>
      <w:r>
        <w:drawing>
          <wps:wsp>
            <wps:txbx>
              <w:txbxContent>
                <w:p><w:r><w:t>Box &bogus; text</w:t></w:r></w:p>
              </w:txbxContent>
            </wps:txbx>
          </wps:wsp>
        </w:drawing>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    assert!(doc.plain_text().contains("Box &bogus; text"));
}

#[test]
fn test_docx_nested_table_malformed_entity_preserves_raw_text() {
    let mut parser = empty_test_parser();
    let xml = r#"<w:tbl xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tr>
    <w:tc>
      <w:tbl>
        <w:tr>
          <w:tc>
            <w:p><w:r><w:t>Inner &bogus; table</w:t></w:r></w:p>
          </w:tc>
        </w:tr>
      </w:tbl>
    </w:tc>
  </w:tr>
</w:tbl>"#;

    let table = parser.parse_table(xml).unwrap();

    assert_eq!(
        table.rows[0].cells[0].nested_tables[0].rows[0].cells[0].plain_text(),
        "Inner &bogus; table"
    );
}

#[test]
fn test_textbox_mc_alternate_content_no_duplication() {
    // mc:AlternateContent with text box in both Choice and Fallback
    // Should only extract once (from Choice branch)
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
            xmlns:v="urn:schemas-microsoft-com:vml">
  <w:body>
    <w:p>
      <w:r>
        <mc:AlternateContent>
          <mc:Choice>
            <w:drawing>
              <wps:wsp>
                <wps:txbx>
                  <w:txbxContent>
                    <w:p>
                      <w:r><w:t>Unique text box</w:t></w:r>
                    </w:p>
                  </w:txbxContent>
                </wps:txbx>
              </wps:wsp>
            </w:drawing>
          </mc:Choice>
          <mc:Fallback>
            <w:pict>
              <v:shape>
                <v:textbox>
                  <w:txbxContent>
                    <w:p>
                      <w:r><w:t>Unique text box</w:t></w:r>
                    </w:p>
                  </w:txbxContent>
                </v:textbox>
              </v:shape>
            </w:pict>
          </mc:Fallback>
        </mc:AlternateContent>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = doc.plain_text();

    // Count occurrences - should appear exactly once
    let count = text.matches("Unique text box").count();
    assert_eq!(
        count, 1,
        "Text box content should appear exactly once, not duplicated. Full text: {}",
        text
    );
}

/// First paragraph's inline images, for VML extraction assertions.
fn first_paragraph_images(doc: &Document) -> &[crate::model::InlineImage] {
    for block in &doc.sections[0].content {
        if let Block::Paragraph(para) = block {
            return &para.images;
        }
    }
    panic!("no paragraph found in first section");
}

#[test]
fn test_vml_pict_image_extracted() {
    // Legacy documents (typically .doc → .docx conversions) embed images
    // as standalone VML: w:pict > v:shape > v:imagedata[@r:id].
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:o="urn:schemas-microsoft-com:office:office"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:r>
        <w:pict>
          <v:shape style="width:100pt;height:50pt">
            <v:imagedata r:id="rId5" o:title="legacy image"/>
          </v:shape>
        </w:pict>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let images = first_paragraph_images(&doc);
    assert_eq!(images.len(), 1, "VML image not extracted");
    assert_eq!(images[0].resource_id, "rId5");
    assert_eq!(images[0].alt_text.as_deref(), Some("legacy image"));
}

#[test]
fn test_shape_picture_fill_is_an_image() {
    // A DrawingML shape filled with a picture (wps:wsp > wps:spPr > a:blipFill) and a
    // VML shape filled with one (v:fill[@r:id]) show that image like a picture does;
    // a VML fill without r:id is a colour and shows none.
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:o="urn:schemas-microsoft-com:office:office"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:drawing><wp:inline><wp:docPr id="1" name="Rectangle 1" descr="Harbour at dusk"/>
      <a:graphic><a:graphicData><wps:wsp><wps:spPr><a:blipFill><a:blip r:embed="rId8"/></a:blipFill></wps:spPr></wps:wsp></a:graphicData></a:graphic>
    </wp:inline></w:drawing></w:r></w:p>
    <w:p><w:r><w:pict>
      <v:rect><v:fill type="frame" r:id="rId9" o:title="Paper texture"/></v:rect>
      <v:rect><v:fill type="solid" color="red"/></v:rect>
    </w:pict></w:r></w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let images: Vec<_> = doc
        .sections
        .iter()
        .flat_map(|s| s.content.iter())
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p),
            _ => None,
        })
        .flat_map(|p| p.images.iter())
        .map(|i| (i.resource_id.as_str(), i.alt_text.as_deref()))
        .collect();
    assert_eq!(
        images,
        [
            ("rId8", Some("Harbour at dusk")),
            ("rId9", Some("Paper texture"))
        ]
    );
}

/// A page background filled with a picture (`w:background` with a VML fill) is drawn on
/// every page: the section references it, the text does not.
#[test]
fn test_page_background_picture_is_referenced_by_the_section() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:background w:color="FFFFFF"><v:background id="_x0000_s1025"><v:fill r:id="rId5" type="frame"/></v:background></w:background>
  <w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:body>
</w:document>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/paper.png"/>
</Relationships>"#;
    let data = create_minimal_docx_with_document_rels(doc_xml, Some(rels));
    let doc = DocxParser::from_bytes(data).unwrap().parse().unwrap();

    assert_eq!(doc.sections[0].background_image.as_deref(), Some("rId5"));
    assert!(doc.sections[0]
        .content
        .iter()
        .all(|b| matches!(b, Block::Paragraph(p) if p.images.is_empty())));

    // A colour background (no picture) references nothing.
    let plain = doc_xml.replace(r#"<v:fill r:id="rId5" type="frame"/>"#, "");
    let data = create_minimal_docx_with_document_rels(&plain, Some(rels));
    let doc = DocxParser::from_bytes(data).unwrap().parse().unwrap();
    assert_eq!(doc.sections[0].background_image, None);
}

#[test]
fn test_vml_object_image_extracted() {
    // Embedded OLE objects (w:object) carry their visual as v:imagedata.
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:r>
        <w:object>
          <v:shape><v:imagedata r:id="rId7"/></v:shape>
        </w:object>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let images = first_paragraph_images(&doc);
    assert_eq!(images.len(), 1, "w:object VML image not extracted");
    assert_eq!(images[0].resource_id, "rId7");
}

#[test]
fn test_vml_fallback_image_not_duplicated() {
    // mc:AlternateContent pairs a DrawingML Choice with a VML Fallback
    // for the same picture — only the Choice branch must produce an image.
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:r>
        <mc:AlternateContent>
          <mc:Choice>
            <w:drawing>
              <a:blip r:embed="rId3"/>
            </w:drawing>
          </mc:Choice>
          <mc:Fallback>
            <w:pict>
              <v:shape><v:imagedata r:id="rId3"/></v:shape>
            </w:pict>
          </mc:Fallback>
        </mc:AlternateContent>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let images = first_paragraph_images(&doc);
    assert_eq!(
        images.len(),
        1,
        "fallback VML must not duplicate the Choice image"
    );
    assert_eq!(images[0].resource_id, "rId3");
}

#[test]
fn test_vml_pict_image_in_table_cell() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:r>
              <w:pict>
                <v:shape><v:imagedata r:id="rId9"/></v:shape>
              </w:pict>
            </w:r>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let table = doc.sections[0]
        .content
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table missing");
    let images = &table.rows[0].cells[0].content[0].images;
    assert_eq!(images.len(), 1, "VML image in table cell not extracted");
    assert_eq!(images[0].resource_id, "rId9");
}

#[test]
fn test_vml_fallback_image_not_duplicated_in_table_cell() {
    // Same AlternateContent dedup contract as the body-paragraph path:
    // the table-cell parser must not extract the Fallback VML copy.
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:r>
              <mc:AlternateContent>
                <mc:Choice>
                  <w:drawing>
                    <a:blip r:embed="rId4"/>
                  </w:drawing>
                </mc:Choice>
                <mc:Fallback>
                  <w:pict>
                    <v:shape><v:imagedata r:id="rId4"/></v:shape>
                  </w:pict>
                </mc:Fallback>
              </mc:AlternateContent>
            </w:r>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let table = doc.sections[0]
        .content
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table missing");
    let images = &table.rows[0].cells[0].content[0].images;
    assert_eq!(
        images.len(),
        1,
        "fallback VML must not duplicate the Choice image in a table cell"
    );
    assert_eq!(images[0].resource_id, "rId4");
}

#[test]
fn test_textbox_multiple_paragraphs() {
    // Text box with multiple paragraphs
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
  <w:body>
    <w:p>
      <w:r>
        <w:drawing>
          <wps:wsp>
            <wps:txbx>
              <w:txbxContent>
                <w:p>
                  <w:r><w:t>First text box paragraph</w:t></w:r>
                </w:p>
                <w:p>
                  <w:r><w:t>Second text box paragraph</w:t></w:r>
                </w:p>
              </w:txbxContent>
            </wps:txbx>
          </wps:wsp>
        </w:drawing>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = doc.plain_text();

    assert!(
        text.contains("First text box paragraph"),
        "Should contain first text box paragraph"
    );
    assert!(
        text.contains("Second text box paragraph"),
        "Should contain second text box paragraph"
    );
}

#[test]
fn test_docx_chart_invalid_numeric_value_propagates_error() {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    let chart_xml = r#"<?xml version="1.0"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart><c:plotArea><c:lineChart>
    <c:ser>
      <c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>S</c:v></c:pt></c:strCache></c:strRef></c:tx>
      <c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strCache></c:strRef></c:cat>
      <c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>not-a-number</c:v></c:pt></c:numCache></c:numRef></c:val>
    </c:ser>
  </c:lineChart></c:plotArea></c:chart>
</c:chartSpace>"#;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <w:body>
    <w:p><w:r><w:drawing>
      <a:graphic><a:graphicData>
        <c:chart r:id="rIdChart"/>
      </a:graphicData></a:graphic>
    </w:drawing></w:r></w:p>
  </w:body>
</w:document>"#;

    let document_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart1.xml"/>
</Relationships>"#;

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(document_rels.as_bytes()).unwrap();

    zip.start_file("word/charts/chart1.xml", options).unwrap();
    zip.write_all(chart_xml.as_bytes()).unwrap();

    let data = zip.finish().unwrap().into_inner();
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let err = parser
        .parse()
        .expect_err("invalid chart numeric value must surface");

    match err {
        Error::InvalidData(msg) => assert!(
            msg.contains("invalid chart numeric value"),
            "unexpected msg: {msg}"
        ),
        other => panic!("expected InvalidData, got {other:?}"),
    }
}

#[test]
fn test_docx_missing_chart_part_propagates_error() {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <w:body>
    <w:p><w:r><w:drawing>
      <a:graphic><a:graphicData>
        <c:chart r:id="rIdChart"/>
      </a:graphicData></a:graphic>
    </w:drawing></w:r></w:p>
  </w:body>
</w:document>"#;

    let document_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="charts/chart1.xml"/>
</Relationships>"#;

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(document_rels.as_bytes()).unwrap();

    let data = zip.finish().unwrap().into_inner();
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let err = parser
        .parse()
        .expect_err("missing referenced chart part must surface");

    match err {
        Error::MissingComponent(path) => assert_eq!(path, "word/charts/chart1.xml"),
        other => panic!("expected MissingComponent, got {other:?}"),
    }
}

#[test]
fn test_docx_body_mixed_entities_preserve_legitimate_and_malformed() {
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>A &amp; B &bogus; C</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let mut parser = DocxParser::from_bytes(buf).expect("parser opens");
    let doc = parser.parse().expect("document parses");
    let text = doc.plain_text();
    assert!(
        text.contains("A & B &bogus; C"),
        "expected legitimate entity decoded and malformed preserved; got {text:?}"
    );
    assert!(
        !text.contains("A &amp; B"),
        "legitimate entity must not remain escaped; got {text:?}"
    );
}

#[test]
fn test_docx_ampersand_midword_renders_intact() {
    // Gate-2: under quick-xml 0.40+, "AT&amp;T" splits into runs "AT","&","T".
    // Verify the fragmentation survives full rendering to Markdown — the entity
    // run carries the same style, so it must concatenate with no inserted
    // separator. Assert on rendered Markdown, not just run existence.
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>AT&amp;T &lt;tag&gt; R &amp; D &#48;&#x30;</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let mut parser = DocxParser::from_bytes(buf).expect("parser opens");
    let doc = parser.parse().expect("document parses");
    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::new())
        .expect("renders to markdown");
    // The `<` of the decoded `<tag>` is escaped in Markdown: written bare, a reader takes
    // `<tag>` for an HTML tag and shows nothing.
    assert!(
        md.contains(r"AT&T \<tag> R & D 00"),
        "entity runs must concatenate without inserted separators; got {md:?}"
    );
}

#[test]
fn test_docx_core_metadata_preserves_fragmented_ampersand() {
    // The core.xml metadata loop previously assigned text per Text event, so
    // under quick-xml 0.40+ "Tom &amp; Jerry" (Text/GeneralRef/Text) kept only
    // the last fragment. Verify element-level accumulation preserves the whole.
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("docProps/core.xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>Tom &amp; Jerry &lt;S1&gt;</dc:title>
  <dc:creator>A &amp; B</dc:creator>
</cp:coreProperties>"#).unwrap();

        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#).unwrap();

        zip.finish().unwrap();
    }

    let mut parser = DocxParser::from_bytes(buf).expect("parser opens");
    let doc = parser.parse().expect("document parses");
    assert_eq!(doc.metadata.title.as_deref(), Some("Tom & Jerry <S1>"));
    assert_eq!(doc.metadata.author.as_deref(), Some("A & B"));
}

#[test]
fn test_vmerge_origin_gets_correct_rowspan() {
    // BUG-2: parse_table must set row_span=2 on the origin cell of a vMerge pair.
    // The parser fix uses col_cursor + vmerge_origins to track this at parse time.
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
          <w:p><w:r><w:t>A</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:p><w:r><w:t>B</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
      <w:tr>
        <w:tc>
          <w:tcPr><w:vMerge/></w:tcPr>
          <w:p/>
        </w:tc>
        <w:tc>
          <w:p><w:r><w:t>C</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let table = match doc.sections[0].content.first() {
        Some(Block::Table(t)) => t,
        other => panic!("expected Block::Table, got {other:?}"),
    };

    assert_eq!(table.rows.len(), 2, "table should have 2 rows");
    // Row 0: origin cell A (row_span=2) + cell B
    assert_eq!(
        table.rows[0].cells[0].row_span, 2,
        "origin cell must have row_span=2"
    );
    // Row 1: only cell C (continuation cell excluded)
    assert_eq!(
        table.rows[1].cells.len(),
        1,
        "continuation cell must be excluded from row 1"
    );
    assert_eq!(table.rows[1].cells[0].plain_text(), "C");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_docx_streaming_emits_expected_events() {
    // FEAT-2: parse_file_streaming must work for DOCX.
    use crate::streaming::{ParseEvent, SectionStreamOptions};
    use std::ops::ControlFlow;

    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Stream me</w:t></w:r></w:p></w:body>
</w:document>"#;

    let data = create_minimal_docx(doc_xml);
    let mut parser = DocxParser::from_bytes(data).unwrap();

    let mut event_log: Vec<&'static str> = Vec::new();
    let mut section_text = String::new();

    parser
        .for_each_section(SectionStreamOptions::default(), |event| {
            match event {
                ParseEvent::DocumentStart { .. } => event_log.push("start"),
                ParseEvent::SectionParsed(s) => {
                    event_log.push("section");
                    section_text = s
                        .content
                        .iter()
                        .filter_map(|b| {
                            if let Block::Paragraph(p) = b {
                                Some(p.plain_text())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                }
                ParseEvent::DocumentEnd => event_log.push("end"),
                _ => {}
            }
            ControlFlow::Continue(())
        })
        .unwrap();

    assert_eq!(event_log, ["start", "section", "end"]);
    assert!(
        section_text.contains("Stream me"),
        "section text missing, got: {section_text}"
    );
}

fn hf_text(paras: &[Paragraph]) -> String {
    paras
        .iter()
        .map(|p| p.plain_text())
        .collect::<Vec<_>>()
        .join("\n")
}

// Bug #3 (verify): text inside a table in a header must be extracted.
#[test]
fn header_extracts_table_cell_text() {
    let xml = r#"<?xml version="1.0"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:tbl>
    <w:tr>
      <w:tc><w:p><w:r><w:t>CellLeft</w:t></w:r></w:p></w:tc>
      <w:tc><w:p><w:r><w:t>CellRight</w:t></w:r></w:p></w:tc>
    </w:tr>
  </w:tbl>
</w:hdr>"#;
    let paras = empty_test_parser().parse_header_footer_xml(xml);
    let text = hf_text(&paras);
    assert!(text.contains("CellLeft"), "missing CellLeft, got: {text:?}");
    assert!(
        text.contains("CellRight"),
        "missing CellRight, got: {text:?}"
    );
}

// Bug #2 (verify): a text box nested inside a header paragraph must not
// swallow the surrounding paragraph's own text, and its text must appear.
#[test]
fn header_textbox_does_not_swallow_surrounding_text() {
    let xml = r#"<?xml version="1.0"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p>
    <w:r><w:t>AnchorBefore</w:t></w:r>
    <w:r><w:pict><v:shape><v:textbox><w:txbxContent>
      <w:p><w:r><w:t>BoxInside</w:t></w:r></w:p>
    </w:txbxContent></v:textbox></v:shape></w:pict></w:r>
    <w:r><w:t>AnchorAfter</w:t></w:r>
  </w:p>
</w:hdr>"#;
    let paras = empty_test_parser().parse_header_footer_xml(xml);
    let text = hf_text(&paras);
    assert!(
        text.contains("AnchorBefore") && text.contains("AnchorAfter"),
        "anchor text lost, got: {text:?}"
    );
    assert!(
        text.contains("BoxInside"),
        "textbox text lost, got: {text:?}"
    );
}

// Baseline: plain header paragraph text still works.
#[test]
fn header_extracts_plain_paragraph_text() {
    let xml = r#"<?xml version="1.0"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>PlainHeader</w:t></w:r></w:p>
</w:hdr>"#;
    let paras = empty_test_parser().parse_header_footer_xml(xml);
    assert_eq!(hf_text(&paras), "PlainHeader");
}

/// Build a minimal DOCX with a document.xml.rels and arbitrary extra parts
/// (e.g. header/footer XML parts referenced from the body's sectPr).
fn create_docx_with_parts(
    document_xml: &str,
    document_rels_xml: &str,
    extra_parts: &[(&str, &str)],
) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(document_rels_xml.as_bytes()).unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();

    for (path, content) in extra_parts {
        zip.start_file(*path, options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }

    zip.finish().unwrap().into_inner()
}

fn section_header_text(section: &crate::model::Section) -> String {
    section
        .header
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|p| p.plain_text())
        .collect::<Vec<_>>()
        .join("\n")
}

// Bug #1: header references of every type (default / first / even) must be
// extracted, not just the "default" one. Previously only `w:type="default"`
// was captured, silently dropping first-page and even-page header text.
#[test]
fn extracts_first_and_even_header_references() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Body</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdHDef"/>
      <w:headerReference w:type="first" r:id="rIdHFirst"/>
      <w:headerReference w:type="even" r:id="rIdHEven"/>
    </w:sectPr>
  </w:body>
</w:document>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHDef" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rIdHFirst" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/>
  <Relationship Id="rIdHEven" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header3.xml"/>
</Relationships>"#;
    let hdr = |text: &str| {
        format!(
            r#"<?xml version="1.0"?><w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:hdr>"#
        )
    };
    let data = create_docx_with_parts(
        document_xml,
        rels,
        &[
            ("word/header1.xml", &hdr("DefaultHeader")),
            ("word/header2.xml", &hdr("FirstPageHeader")),
            ("word/header3.xml", &hdr("EvenPageHeader")),
        ],
    );

    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = section_header_text(&doc.sections[0]);

    assert!(
        text.contains("DefaultHeader"),
        "missing default, got: {text:?}"
    );
    assert!(
        text.contains("FirstPageHeader"),
        "missing first-page, got: {text:?}"
    );
    assert!(
        text.contains("EvenPageHeader"),
        "missing even-page, got: {text:?}"
    );
}

// Bug: header/footer references inside a *section-break* sectPr (nested in a
// paragraph's `w:pPr`) were dropped — only the final body-level `w:sectPr`
// was read. Multi-section documents place each section's header/footer refs
// in the paragraph-level sectPr that terminates that section, so all but the
// last section's header/footer text was silently lost.
#[test]
fn extracts_header_footer_from_section_break_sectpr() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Section one body</w:t></w:r></w:p>
    <w:p>
      <w:pPr>
        <w:sectPr>
          <w:headerReference w:type="default" r:id="rIdH1"/>
          <w:footerReference w:type="default" r:id="rIdF1"/>
        </w:sectPr>
      </w:pPr>
    </w:p>
    <w:p><w:r><w:t>Section two body</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdH2"/>
    </w:sectPr>
  </w:body>
</w:document>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdH1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rIdF1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
  <Relationship Id="rIdH2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header2.xml"/>
</Relationships>"#;
    let hdr = |tag: &str, text: &str| {
        format!(
            r#"<?xml version="1.0"?><w:{tag} xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:{tag}>"#
        )
    };
    let data = create_docx_with_parts(
        document_xml,
        rels,
        &[
            ("word/header1.xml", &hdr("hdr", "SectionOneHeader")),
            ("word/footer1.xml", &hdr("ftr", "SectionOneFooter")),
            ("word/header2.xml", &hdr("hdr", "SectionTwoHeader")),
        ],
    );

    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let section = &doc.sections[0];
    let header = section_header_text(section);
    let footer = section
        .footer
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|p| p.plain_text())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        header.contains("SectionOneHeader"),
        "section-break header dropped, got: {header:?}"
    );
    assert!(
        header.contains("SectionTwoHeader"),
        "body-level header dropped, got: {header:?}"
    );
    assert!(
        footer.contains("SectionOneFooter"),
        "section-break footer dropped, got: {footer:?}"
    );
}

// A header/footer whose text lives in a *nested* table must not lose the
// inner table's text. The flat header/footer model discards table structure
// (by design), but every cell's text — at any nesting depth — must survive.
#[test]
fn extracts_nested_table_text_in_header() {
    let inner = r#"<?xml version="1.0"?><w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:tbl><w:tr><w:tc><w:p><w:r><w:t>OuterCellText</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>InnerCellText</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:tc></w:tr></w:tbl></w:hdr>"#;
    let paras = empty_test_parser().parse_header_footer_xml(inner);
    let text = hf_text(&paras);
    assert!(
        text.contains("OuterCellText"),
        "outer cell text dropped, got: {text:?}"
    );
    assert!(
        text.contains("InnerCellText"),
        "nested table text dropped, got: {text:?}"
    );
}

// A single header referenced by several section types (default + first) must
// be parsed once, not duplicated.
#[test]
fn deduplicates_shared_header_reference() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Body</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdShared"/>
      <w:headerReference w:type="first" r:id="rIdShared"/>
    </w:sectPr>
  </w:body>
</w:document>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdShared" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
</Relationships>"#;
    let data = create_docx_with_parts(
        document_xml,
        rels,
        &[(
            "word/header1.xml",
            r#"<?xml version="1.0"?><w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>SharedHeader</w:t></w:r></w:p></w:hdr>"#,
        )],
    );

    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = section_header_text(&doc.sections[0]);

    assert_eq!(
        text, "SharedHeader",
        "shared header must appear exactly once"
    );
}

// A header's relationship IDs live in its own rels part
// (word/_rels/header1.xml.rels), independently numbered from the body's.
// A hyperlink r:id must resolve against the header's rels, not document.xml's
// — otherwise a colliding rId silently yields the wrong URL.
#[test]
fn header_hyperlink_resolves_against_its_own_rels() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Body</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdHeaderPart"/>
    </w:sectPr>
  </w:body>
</w:document>"#;
    // document.xml.rels: rId1 is a DECOY that points at a body URL.
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHeaderPart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://body-url.example/" TargetMode="External"/>
</Relationships>"#;
    let header_xml = r#"<?xml version="1.0"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:p><w:hyperlink r:id="rId1"><w:r><w:t>HeaderLink</w:t></w:r></w:hyperlink></w:p>
</w:hdr>"#;
    // header1.xml.rels: the SAME rId1 points at the correct header URL.
    let header_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://header-url.example/" TargetMode="External"/>
</Relationships>"#;

    let data = create_docx_with_parts(
        document_xml,
        doc_rels,
        &[
            ("word/header1.xml", header_xml),
            ("word/_rels/header1.xml.rels", header_rels),
        ],
    );

    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let header = doc.sections[0].header.as_ref().expect("header present");
    let link = header
        .iter()
        .flat_map(|p| p.runs.iter())
        .find_map(|r| r.hyperlink.clone())
        .expect("header hyperlink resolved");

    assert_eq!(
        link, "https://header-url.example/",
        "header hyperlink must resolve against the header's own rels"
    );
}

// Header/footer text is auxiliary and must not enter the document heading
// outline, even when it carries a Heading paragraph style — otherwise it
// would emit a stray `#` inside the rendered header blockquote and mislead
// structure-aware consumers.
#[test]
fn header_paragraph_is_not_treated_as_a_heading() {
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p><w:r><w:t>Body</w:t></w:r></w:p>
    <w:sectPr>
      <w:headerReference w:type="default" r:id="rIdHdr"/>
    </w:sectPr>
  </w:body>
</w:document>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdHdr" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
</Relationships>"#;
    let styles = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="Heading 1"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
</w:styles>"#;
    let header_xml = r#"<?xml version="1.0"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>HeaderTitle</w:t></w:r></w:p>
</w:hdr>"#;
    let data = create_docx_with_parts(
        document_xml,
        rels,
        &[
            ("word/styles.xml", styles),
            ("word/header1.xml", header_xml),
        ],
    );

    let mut parser = DocxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let header = doc.sections[0].header.as_ref().expect("header present");

    assert_eq!(header[0].plain_text(), "HeaderTitle");
    assert!(
        !header[0].heading.is_heading(),
        "header paragraph must not be a document heading, got {:?}",
        header[0].heading
    );
}
