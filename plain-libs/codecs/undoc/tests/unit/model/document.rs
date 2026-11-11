use super::*;
use crate::detect::FormatType;
use crate::model::{Cell, RevisionType, Row, TextRun, TextStyle};

#[test]
fn test_document_default_format_is_docx() {
    let doc = Document::new();
    assert_eq!(doc.format, FormatType::Docx);
}

#[test]
fn test_document_creation() {
    let mut doc = Document::new();
    assert!(doc.is_empty());

    let mut section = Section::new(0);
    let para = Paragraph {
        runs: vec![TextRun::plain("Hello, World!")],
        ..Default::default()
    };
    section.add_paragraph(para);
    doc.add_section(section);

    assert!(!doc.is_empty());
    assert_eq!(doc.total_blocks(), 1);
}

#[test]
fn test_plain_text_extraction() {
    let mut doc = Document::new();
    let mut section = Section::new(0);

    section.add_paragraph(Paragraph {
        runs: vec![
            TextRun::plain("Hello, "),
            TextRun {
                text: "World".to_string(),
                style: TextStyle {
                    bold: true,
                    ..Default::default()
                },
                hyperlink: None,
                line_break: false,
                page_break: false,
                revision: RevisionType::None,
                note: None,
            },
            TextRun::plain("!"),
        ],
        ..Default::default()
    });

    doc.add_section(section);
    assert_eq!(doc.plain_text(), "Hello, World!");
}

#[test]
fn test_plain_text_preserves_boundary_spaces() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::with_text("  padded text  "));
    doc.add_section(section);

    assert_eq!(doc.plain_text(), "  padded text  ");
}

#[test]
fn test_metadata_serialization() {
    let meta = Metadata {
        title: Some("Test Document".to_string()),
        author: Some("Test Author".to_string()),
        ..Default::default()
    };

    let json = plain_libs::serde_json::to_string(&meta).unwrap();
    assert!(json.contains("Test Document"));
    assert!(json.contains("Test Author"));
    // Empty fields should not be serialized
    assert!(!json.contains("subject"));
}

/// A one-cell table holding `text`, its cell holding `nested`.
fn table(text: &str, nested: Vec<Table>) -> Table {
    let mut cell = Cell::with_text(text);
    cell.nested_tables = nested;
    let mut row = Row::new();
    row.add_cell(cell);
    let mut table = Table::new();
    table.add_row(row);
    table
}

#[test]
fn tables_are_numbered_per_section_with_nested_tables_after_their_holder() {
    let mut doc = Document::new();
    let mut first = Section::new(0);
    first.add_table(table(
        "a",
        vec![
            table("a.1", vec![table("a.1.1", vec![])]),
            table("a.2", vec![]),
        ],
    ));
    first.add_paragraph(Paragraph::with_text("between"));
    first.add_table(table("b", vec![]));
    doc.add_section(first);
    doc.add_section(Section::new(1));
    let mut third = Section::new(2);
    third.add_table(table("c", vec![]));
    doc.add_section(third);

    let found: Vec<(usize, usize, String)> = doc
        .tables()
        .map(|(s, t, table)| (s, t, table.rows[0].cells[0].plain_text()))
        .collect();
    let expected = [
        (1, 1, "a"),
        (1, 2, "a.1"),
        (1, 3, "a.1.1"),
        (1, 4, "a.2"),
        (1, 5, "b"),
        (3, 1, "c"),
    ]
    .map(|(s, t, text)| (s, t, text.to_string()));
    assert_eq!(found, expected);
    assert_eq!(Document::new().tables().count(), 0);
}

#[test]
fn test_section_with_name() {
    let section = Section::with_name(0, "Sheet1");
    assert_eq!(section.name, Some("Sheet1".to_string()));
    assert_eq!(section.index, 0);
}
