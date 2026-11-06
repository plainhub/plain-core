use super::*;

#[test]
fn input_read_stops_at_the_limit_without_trusting_declared_lengths() {
    assert!(read_input(std::io::repeat(0)).is_err());
    assert_eq!(read_input(&b"document"[..]).unwrap(), b"document");
}

#[test]
fn rejects_deep_xml_dtd_and_unclosed_elements() {
    let xml = format!(
        "{}{}",
        "<a>".repeat(MAX_XML_DEPTH + 1),
        "</a>".repeat(MAX_XML_DEPTH + 1)
    );
    assert!(check_xml(&xml).is_err());
    assert!(check_xml("<!DOCTYPE a [<!ENTITY x 'text'>]><a>&x;</a>").is_err());
    assert!(check_xml("<a>").is_err());
    assert!(check_xml("<a><b>text &amp; text</b></a>").is_ok());
}

#[test]
fn rejects_large_table_grids_before_render_allocation() {
    let cell = crate::Cell {
        col_span: 1000,
        ..Default::default()
    };
    let row = crate::Row {
        cells: vec![cell; 1001],
        ..Default::default()
    };
    let table = crate::Table {
        rows: vec![row; 2],
        ..Default::default()
    };
    let section = crate::Section {
        content: vec![crate::Block::Table(table)],
        ..Default::default()
    };
    let document = crate::Document {
        sections: vec![section],
        ..Default::default()
    };
    assert!(check_document(&document).is_err());
}

#[test]
fn rejects_vertical_merges_that_expand_the_actual_grid() {
    let cell = crate::Cell {
        col_span: 1000,
        row_span: 1000,
        ..Default::default()
    };
    let row = crate::Row {
        cells: vec![cell],
        ..Default::default()
    };
    let table = crate::Table {
        rows: vec![row; 100],
        ..Default::default()
    };
    assert!(check_table(&table, 0).is_err());
}
