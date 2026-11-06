use super::*;
use std::io::{Cursor, Write};

fn create_test_zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, content) in entries {
        zip.start_file(*name, options).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn test_parser() -> XlsxParser {
    let container = OoxmlContainer::from_bytes(create_test_zip(&[])).unwrap();
    XlsxParser {
        container,
        shared_strings: SharedStrings::default(),
        styles: Styles::default(),
        sheets: Vec::new(),
        relationships: HashMap::new(),
    }
}

#[test]
fn test_parse_sheet_preserves_sparse_cell_positions() {
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>left</t></is></c>
                        <c r="C1" t="inlineStr"><is><t>right</t></is></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].cells.len(), 3);
    assert_eq!(table.rows[0].cells[0].plain_text(), "left");
    assert_eq!(table.rows[0].cells[1].plain_text(), "");
    assert_eq!(table.rows[0].cells[2].plain_text(), "right");
}

#[test]
fn test_parse_sheet_inline_string_entities_and_cr_refs() {
    // Inline-string cell exercising the cell-value loop's GeneralRef arm and
    // the flush-point CR normalization: entities decode, a CRLF pair encoded
    // as two &#13;&#10; refs collapses to a single LF.
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>a&#13;&#10;b &amp; c &bogus; d</t></is></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows[0].cells[0].plain_text(), "a\nb & c &bogus; d");
}

#[test]
fn test_parse_sheet_stray_ampersand_in_skipped_element_does_not_abort() {
    // A raw `&` (ill-formed XML from a non-conforming producer) inside a
    // <f> formula — which the loop skips — must not abort the whole sheet.
    // reader_for enables allow_dangling_amp so tokenization degrades
    // gracefully instead of returning Err(IllFormed(UnclosedReference)).
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1"><f>IF(a&b,1,0)</f><v>5</v></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .expect("stray & must not abort parsing");
    assert_eq!(table.rows[0].cells[0].plain_text(), "5");
}

#[test]
fn test_parse_sheet_trims_formula_only_trailing_row() {
    // The parser does not retain formula presence on the cell model: a formula-only
    // cell with no computed <v> is content-empty. Under issue #6's fix it is therefore
    // trimmed together with any other trailing content-empty row.
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1"><f>SUM(B1:C1)</f></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows.len(), 0);
}

#[test]
fn test_parse_sheet_trims_trailing_rows_with_explicit_empty_cells() {
    // Reproduces issue #6: rows with explicit empty cell references (e.g. from
    // conditional formatting) must be trimmed from the trailing end.
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>data</t></is></c>
                    </row>
                    <row r="2">
                        <c r="A2"/>
                        <c r="B2"/>
                    </row>
                    <row r="3">
                        <c r="A3"/>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(
        table.rows.len(),
        1,
        "trailing empty-cell rows must be trimmed"
    );
    assert_eq!(table.rows[0].cells[0].plain_text(), "data");
}

#[test]
fn test_parse_sheet_preserves_formula_only_sparse_middle_cell_position() {
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>left</t></is></c>
                        <c r="C1"><f>SUM(A1:B1)</f></c>
                        <c r="E1" t="inlineStr"><is><t>right</t></is></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].cells.len(), 5);
    assert_eq!(table.rows[0].cells[0].plain_text(), "left");
    assert_eq!(table.rows[0].cells[1].plain_text(), "");
    assert_eq!(table.rows[0].cells[2].plain_text(), "");
    assert_eq!(table.rows[0].cells[3].plain_text(), "");
    assert_eq!(table.rows[0].cells[4].plain_text(), "right");
}

#[test]
fn test_parse_sheet_preserves_explicit_empty_cell_position() {
    // An explicit empty cell in a MIDDLE position (not trailing) must be preserved.
    // Trailing empty columns are trimmed by the post-parse column-trim pass, but
    // interior gaps between content cells must remain to keep column alignment.
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>value</t></is></c>
                        <c r="C1"/>
                        <c r="E1" t="inlineStr"><is><t>end</t></is></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows.len(), 1);
    // E1 (index 4) is the rightmost content cell → keep = 5
    // Interior gaps B1 and C1/D1 are preserved.
    assert_eq!(table.rows[0].cells.len(), 5);
    assert_eq!(table.rows[0].cells[0].plain_text(), "value");
    assert_eq!(table.rows[0].cells[1].plain_text(), "");
    assert_eq!(table.rows[0].cells[2].plain_text(), "");
    assert_eq!(table.rows[0].cells[3].plain_text(), "");
    assert_eq!(table.rows[0].cells[4].plain_text(), "end");
}

#[test]
fn test_parse_sheet_trims_trailing_empty_columns() {
    // Columns C, D (gap), E contain only empty cells — they should be trimmed.
    // Data is only in columns A and B.
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>H1</t></is></c>
                        <c r="B1" t="inlineStr"><is><t>H2</t></is></c>
                        <c r="D1"/>
                        <c r="E1"/>
                    </row>
                    <row r="2">
                        <c r="A2" t="inlineStr"><is><t>val</t></is></c>
                        <c r="B2" t="inlineStr"><is><t>42</t></is></c>
                        <c r="D2"/>
                        <c r="E2"/>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows.len(), 2);
    // Rightmost non-empty column is B (index 1), so each row should have 2 cells.
    // Note: push_cell_with_row_local_spacing adds gap cell for C before D,
    // so before trim each row has [A, B, gap-C, D, E] = 5 cells.
    assert_eq!(
        table.rows[0].cells.len(),
        2,
        "trailing empty columns (C gap, D, E) must be trimmed; got {} cells",
        table.rows[0].cells.len()
    );
    assert_eq!(table.rows[0].cells[0].plain_text(), "H1");
    assert_eq!(table.rows[0].cells[1].plain_text(), "H2");
    assert_eq!(table.rows[1].cells.len(), 2);
}

#[test]
fn test_parse_sheet_preserves_middle_empty_row() {
    // Empty rows between data rows must NOT be trimmed — only trailing ones.
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="inlineStr"><is><t>first</t></is></c>
                    </row>
                    <row r="2">
                        <c r="A2"/>
                    </row>
                    <row r="3">
                        <c r="A3" t="inlineStr"><is><t>third</t></is></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(
        table.rows.len(),
        3,
        "middle empty row must be preserved, only trailing empty rows are trimmed"
    );
    assert_eq!(table.rows[0].cells[0].plain_text(), "first");
    assert_eq!(table.rows[2].cells[0].plain_text(), "third");
}

/// The fuzzer's worksheet, cut down: a cell whose `r` is a long run of letters. It
/// panicked with overflow checks; it is a cell in the next column.
#[test]
fn a_cell_whose_reference_is_not_a_cell_takes_the_next_column() {
    let parser = test_parser();
    let sheet_xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
            <sheetData><row r="1">
                <c r="A1" t="inlineStr"><is><t>a</t></is></c>
                <c r="application/vnd.openxmlformats-officedocument.wordprocessingml.dB2"><v>1</v></c>
                <c r="ZZZZZZ1"><v>2</v></c>
            </row></sheetData></worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    let texts: Vec<String> = table.rows[0].cells.iter().map(|c| c.plain_text()).collect();
    assert_eq!(texts, ["a", "1", "2"]);
}

#[test]
fn test_parse_sheet_sparse_high_column_reconstruction_is_row_local() {
    let parser = test_parser();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="BA1" t="inlineStr"><is><t>far</t></is></c>
                    </row>
                    <row r="2">
                        <c r="A2" t="inlineStr"><is><t>near</t></is></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let table = parser
        .parse_sheet(sheet_xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap();

    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0].cells.len(), 53);
    assert_eq!(table.rows[0].cells[52].plain_text(), "far");
    assert_eq!(table.rows[1].cells.len(), 1);
    assert_eq!(table.rows[1].cells[0].plain_text(), "near");
}

#[test]
fn test_parse_cell_ref() {
    // Test cell reference parsing
    assert_eq!(XlsxParser::parse_cell_ref("A1"), Some((0, 1)));
    assert_eq!(XlsxParser::parse_cell_ref("B2"), Some((1, 2)));
    assert_eq!(XlsxParser::parse_cell_ref("Z1"), Some((25, 1)));
    assert_eq!(XlsxParser::parse_cell_ref("AA1"), Some((26, 1)));
    assert_eq!(XlsxParser::parse_cell_ref("AB1"), Some((27, 1)));
    assert_eq!(XlsxParser::parse_cell_ref("AZ1"), Some((51, 1)));
    assert_eq!(XlsxParser::parse_cell_ref("BA1"), Some((52, 1)));
    assert_eq!(XlsxParser::parse_cell_ref("ab3"), Some((27, 3)));
    assert_eq!(
        XlsxParser::parse_cell_ref("XFD1048576"),
        Some((16_383, 1_048_576))
    );
}

/// A reference outside the A1 form or past the sheet is no position. The first is a
/// fuzzer's: its letters overflowed the column (a panic with overflow checks, a wrapped
/// column of billions without them).
#[test]
fn a_reference_that_is_not_a_cell_is_no_position() {
    for reference in [
        "application/vnd.openxmlformats-officedocument.wordprocessingml.dB2",
        "XFE1",
        "ZZZZ1",
        "A0",
        "A1048577",
        "A99999999999",
        "1A",
        "A1B",
        "A",
        "1",
        "",
        "$A$1",
    ] {
        assert_eq!(XlsxParser::parse_cell_ref(reference), None, "{reference}");
    }
}

#[test]
fn test_date_formatting() {
    // Test that styles are correctly parsed and dates are formatted
    use crate::xlsx::styles::Styles;

    // Test parsing styles.xml content
    let styles_xml = r#"<?xml version="1.0"?>
            <styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <numFmts count="1">
                    <numFmt numFmtId="177" formatCode="mmmm\ d\,\ yyyy"/>
                </numFmts>
                <cellXfs count="2">
                    <xf numFmtId="0"/>
                    <xf numFmtId="177"/>
                </cellXfs>
            </styleSheet>"#;

    let styles = Styles::parse(styles_xml);

    // Style index 1 should have numFmtId 177 (date format)
    assert_eq!(styles.get_num_fmt_id(1), Some(177));
    assert!(styles.is_date_format(177));

    // Test date conversion
    assert_eq!(
        Styles::serial_to_date(44197.0),
        Some("2021-01-01".to_string())
    );
}

#[test]
fn test_rels_path_for() {
    assert_eq!(
        XlsxParser::rels_path_for("xl/worksheets/sheet1.xml"),
        "xl/worksheets/_rels/sheet1.xml.rels"
    );
    assert_eq!(
        XlsxParser::rels_path_for("xl/drawings/drawing1.xml"),
        "xl/drawings/_rels/drawing1.xml.rels"
    );
}

#[test]
fn test_resolve_relative_path() {
    assert_eq!(
        XlsxParser::resolve_relative_path("xl/worksheets", "../drawings/drawing1.xml"),
        "xl/drawings/drawing1.xml"
    );
    assert_eq!(
        XlsxParser::resolve_relative_path("xl/drawings", "../media/image1.png"),
        "xl/media/image1.png"
    );
    assert_eq!(
        XlsxParser::resolve_relative_path("", "xl/media/image1.png"),
        "xl/media/image1.png"
    );
}

/// Shared-string cells resolve to their text, the sheet is listed by name before
/// parsing, and only the first row of the sheet is marked as the header.
#[test]
fn test_shared_strings_sheet_names_and_header_row() {
    let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    let shared_strings = r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="4" uniqueCount="4">
  <si><t>First Name</t></si><si><t>Last Name</t></si><si><t>Ada</t></si><si><t>Lovelace</t></si>
</sst>"#;
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row>
    <row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2" t="s"><v>3</v></c></row>
  </sheetData>
</worksheet>"#;
    let data =
        create_minimal_xlsx_with_parts(Some(workbook_rels), None, Some(shared_strings), sheet_xml);

    let mut parser = XlsxParser::from_bytes(data).unwrap();
    assert_eq!(parser.sheet_names(), ["Sheet1"]);

    let doc = parser.parse().unwrap();
    let table = doc.sections[0]
        .content
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("the sheet becomes a table");
    let texts: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| row.cells.iter().map(|cell| cell.plain_text()).collect())
        .collect();
    assert_eq!(texts, [["First Name", "Last Name"], ["Ada", "Lovelace"]]);
    assert!(table.rows[0].is_header);
    assert!(!table.rows[1].is_header);
}

fn create_minimal_xlsx(workbook_rels_xml: Option<&str>, sheet_rels_xml: Option<&str>) -> Vec<u8> {
    create_minimal_xlsx_with_parts(
        workbook_rels_xml,
        sheet_rels_xml,
        None,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Hello</t></is></c>
    </row>
  </sheetData>
</worksheet>"#,
    )
}

fn create_minimal_xlsx_with_parts(
    workbook_rels_xml: Option<&str>,
    sheet_rels_xml: Option<&str>,
    shared_strings_xml: Option<&str>,
    sheet_xml: &str,
) -> Vec<u8> {
    use std::io::{Cursor, Write};

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
</Types>"#,
        )
        .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
        )
        .unwrap();

    if let Some(workbook_rels_xml) = workbook_rels_xml {
        zip.start_file("xl/_rels/workbook.xml.rels", options)
            .unwrap();
        zip.write_all(workbook_rels_xml.as_bytes()).unwrap();
    }

    zip.start_file("xl/workbook.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    )
    .unwrap();

    if let Some(sheet_rels_xml) = sheet_rels_xml {
        zip.start_file("xl/worksheets/_rels/sheet1.xml.rels", options)
            .unwrap();
        zip.write_all(sheet_rels_xml.as_bytes()).unwrap();
    }

    if let Some(shared_strings_xml) = shared_strings_xml {
        zip.start_file("xl/sharedStrings.xml", options).unwrap();
        zip.write_all(shared_strings_xml.as_bytes()).unwrap();
    }

    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    zip.write_all(sheet_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

#[test]
fn test_parse_relationships() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
                <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/printerSettings" Target="../printerSettings/printerSettings1.bin"/>
            </Relationships>"#;

    let rels = XlsxParser::parse_relationships(xml);
    assert_eq!(rels.len(), 2);

    let (rel_type, target) = rels.get("rId1").unwrap();
    assert!(rel_type.contains("drawing"));
    assert_eq!(target, "../drawings/drawing1.xml");

    let (rel_type, _) = rels.get("rId2").unwrap();
    assert!(rel_type.contains("printerSettings"));
}

#[test]
fn test_cell_carriage_returns_do_not_break_markdown_table() {
    // Excel/openpyxl store in-cell CR and CRLF as &#13; character
    // references. A bare CR leaking into a Markdown pipe table splits
    // the row across physical lines (issue #7).
    let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>h1</t></is></c>
      <c r="B1" t="inlineStr"><is><t>h2</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>cr</t></is></c>
      <c r="B2" t="inlineStr"><is><t>line one&#13;line two</t></is></c>
    </row>
    <row r="3">
      <c r="A3" t="inlineStr"><is><t>crlf</t></is></c>
      <c r="B3" t="inlineStr"><is><t>line one&#13;&#10;line two</t></is></c>
    </row>
  </sheetData>
</worksheet>"#;
    let data = create_minimal_xlsx_with_parts(Some(workbook_rels), None, None, sheet_xml);

    let mut parser = XlsxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();

    assert!(!md.contains('\r'), "bare CR leaked into markdown: {md:?}");
    for label in ["cr", "crlf"] {
        let row_line = md
            .lines()
            .find(|l| l.starts_with(&format!("| {label} ")))
            .unwrap_or_else(|| panic!("row '{label}' missing in:\n{md}"));
        assert!(
            row_line.contains("line one") && row_line.contains("line two"),
            "row '{label}' split across lines:\n{md}"
        );
    }
}

/// A worksheet around `body` (its `<sheetData>`/`<mergeCells>` children).
fn worksheet(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">{body}</worksheet>"#
    )
}

fn sheet_table(body: &str) -> Table {
    let xml = worksheet(body);
    test_parser()
        .parse_sheet(&xml, &HashMap::new(), &HashMap::new(), &HashMap::new())
        .unwrap()
}

fn row_texts(table: &Table) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.plain_text()).collect())
        .collect()
}

/// The column count of every pipe-table line in `md`.
fn markdown_table_widths(md: &str) -> Vec<usize> {
    md.lines()
        .filter(|l| l.starts_with('|'))
        .map(|l| l.matches('|').count() - 1)
        .collect()
}

fn sheet_markdown(body: &str) -> String {
    let sheet_xml = worksheet(body);
    let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    let data = create_minimal_xlsx_with_parts(Some(workbook_rels), None, None, &sheet_xml);
    let doc = XlsxParser::from_bytes(data).unwrap().parse().unwrap();
    crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap()
}

/// A vertical merge whose covered position the worksheet writes as a cell of its own
/// (Excel does when the range is styled). The model must record the merge once — on
/// its owner — or the renderer pads the covered column a second time and every value
/// to its right shifts out from under its heading.
const VERTICAL_MERGE_SHEET: &str = r#"
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Group</t></is></c>
      <c r="B1" t="inlineStr"><is><t>Item</t></is></c>
      <c r="C1" t="inlineStr"><is><t>Step</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>A</t></is></c>
      <c r="B2" t="inlineStr"><is><t>x</t></is></c>
      <c r="C2" t="inlineStr"><is><t>1. install</t></is></c>
    </row>
    <row r="3">
      <c r="A3" s="1"/>
      <c r="B3" t="inlineStr"><is><t>y</t></is></c>
      <c r="C3" t="inlineStr"><is><t>2. upgrade</t></is></c>
    </row>
    <row r="4">
      <c r="A4" t="inlineStr"><is><t>B</t></is></c>
      <c r="B4" t="inlineStr"><is><t>z</t></is></c>
      <c r="C4" t="inlineStr"><is><t>3. verify</t></is></c>
    </row>
  </sheetData>
  <mergeCells count="1"><mergeCell ref="A2:A3"/></mergeCells>"#;

#[test]
fn test_vertical_merge_covered_cell_is_not_a_cell() {
    let table = sheet_table(VERTICAL_MERGE_SHEET);
    assert_eq!(
        row_texts(&table),
        vec![
            vec!["Group", "Item", "Step"],
            vec!["A", "x", "1. install"],
            vec!["y", "2. upgrade"],
            vec!["B", "z", "3. verify"],
        ]
    );
    assert_eq!(table.rows[1].cells[0].row_span, 2);
    assert_eq!(table.cell_columns()[2], vec![1, 2]);
}

#[test]
fn test_vertical_merge_renders_one_column_per_grid_column() {
    let md = sheet_markdown(VERTICAL_MERGE_SHEET);
    let widths = markdown_table_widths(&md);
    assert!(!widths.is_empty(), "no table in:\n{md}");
    assert!(widths.iter().all(|&w| w == 3), "{widths:?} in:\n{md}");
    assert!(
        md.lines()
            .any(|l| l.contains("| y |") && l.trim_end().ends_with("| 2. upgrade |")),
        "the row under the merge must keep its values under their headings:\n{md}"
    );
}

#[test]
fn test_horizontal_merge_covered_cell_is_not_a_cell() {
    let table = sheet_table(
        r#"<sheetData>
                <row r="1">
                    <c r="A1" t="inlineStr"><is><t>Span</t></is></c>
                    <c r="B1" s="1"/>
                    <c r="C1" t="inlineStr"><is><t>C</t></is></c>
                </row>
                <row r="2">
                    <c r="A2" t="inlineStr"><is><t>a</t></is></c>
                    <c r="B2" t="inlineStr"><is><t>b</t></is></c>
                    <c r="C2" t="inlineStr"><is><t>c</t></is></c>
                </row>
            </sheetData>
            <mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>"#,
    );
    assert_eq!(
        row_texts(&table),
        vec![vec!["Span", "C"], vec!["a", "b", "c"]]
    );
    assert_eq!(table.rows[0].cells[0].col_span, 2);
    assert_eq!(table.column_count(), 3);
}

#[test]
fn test_unwritten_merge_owner_gets_its_spans() {
    // B2:B3 merged and empty; the worksheet writes neither B2 nor B3.
    let table = sheet_table(
        r#"<sheetData>
                <row r="1">
                    <c r="A1" t="inlineStr"><is><t>a</t></is></c>
                    <c r="B1" t="inlineStr"><is><t>b</t></is></c>
                    <c r="C1" t="inlineStr"><is><t>c</t></is></c>
                </row>
                <row r="2">
                    <c r="A2" t="inlineStr"><is><t>1</t></is></c>
                    <c r="C2" t="inlineStr"><is><t>3</t></is></c>
                </row>
                <row r="3">
                    <c r="A3" t="inlineStr"><is><t>4</t></is></c>
                    <c r="C3" t="inlineStr"><is><t>6</t></is></c>
                </row>
            </sheetData>
            <mergeCells count="1"><mergeCell ref="B2:B3"/></mergeCells>"#,
    );
    assert_eq!(
        row_texts(&table),
        vec![vec!["a", "b", "c"], vec!["1", "", "3"], vec!["4", "6"]]
    );
    assert_eq!(table.rows[1].cells[1].row_span, 2);
    assert_eq!(table.cell_columns()[2], vec![0, 2]);
}

#[test]
fn test_merge_over_an_omitted_row_spans_only_present_rows() {
    // A2:A4 merged, but the worksheet omits row 3 entirely: the table has two rows
    // under the merge's top, not three.
    let table = sheet_table(
        r#"<sheetData>
                <row r="2">
                    <c r="A2" t="inlineStr"><is><t>G</t></is></c>
                    <c r="B2" t="inlineStr"><is><t>x</t></is></c>
                </row>
                <row r="4">
                    <c r="B4" t="inlineStr"><is><t>z</t></is></c>
                </row>
                <row r="5">
                    <c r="A5" t="inlineStr"><is><t>H</t></is></c>
                    <c r="B5" t="inlineStr"><is><t>w</t></is></c>
                </row>
            </sheetData>
            <mergeCells count="1"><mergeCell ref="A2:A4"/></mergeCells>"#,
    );
    assert_eq!(table.rows[0].cells[0].row_span, 2);
    assert_eq!(
        table.cell_columns(),
        vec![vec![0, 1], vec![1], vec![0, 1]],
        "row 5 must start in column 0 — the merge does not reach it"
    );
}

#[test]
fn test_trailing_column_trim_measures_grid_columns() {
    // Row 2 sits under a vertical merge, so its cells start one column right; its
    // last content reaches column C. Trimming by index in the row would cut it.
    let table = sheet_table(
        r#"<sheetData>
                <row r="1">
                    <c r="A1" t="inlineStr"><is><t>G</t></is></c>
                    <c r="B1" t="inlineStr"><is><t>b</t></is></c>
                    <c r="C1" s="1"/>
                    <c r="D1" s="1"/>
                </row>
                <row r="2">
                    <c r="B2" t="inlineStr"><is><t>y</t></is></c>
                    <c r="C2" t="inlineStr"><is><t>z</t></is></c>
                    <c r="D2" s="1"/>
                </row>
            </sheetData>
            <mergeCells count="1"><mergeCell ref="A1:A2"/></mergeCells>"#,
    );
    // Row 1's styled empty `C1` is not placed (no content past it); the grid still spans
    // column C because row 2 reaches it.
    assert_eq!(row_texts(&table), vec![vec!["G", "b"], vec!["y", "z"]]);
    assert_eq!(table.column_count(), 3);
}

/// A drawing's picture with an SVG original: the PNG is the primary and carries the
/// picture's description, the SVG is its alternate.
#[test]
fn test_drawing_picture_companions_are_marked() {
    let drawing = r#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<xdr:oneCellAnchor><xdr:pic><xdr:nvPicPr><xdr:cNvPr id="2" name="Picture 1" descr="Logo"/><xdr:cNvPicPr/></xdr:nvPicPr>
<xdr:blipFill><a:blip r:embed="rId1"><a:extLst><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="rId2"/></a:ext></a:extLst></a:blip></xdr:blipFill></xdr:pic></xdr:oneCellAnchor>
</xdr:wsDr>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image2.svg"/>
</Relationships>"#;
    let mut parser = test_parser();
    parser.container = OoxmlContainer::from_bytes(create_test_zip(&[
        ("xl/drawings/drawing1.xml", drawing),
        ("xl/drawings/_rels/drawing1.xml.rels", rels),
        ("xl/media/image1.png", "png"),
        ("xl/media/image2.svg", "<svg/>"),
    ]))
    .unwrap();
    let mut doc = Document::new();
    parser.extract_resources(&mut doc).unwrap();

    let png = &doc.resources["image1.png"];
    assert_eq!(png.role, ResourceRole::Primary);
    assert_eq!(png.alt_text.as_deref(), Some("Logo"));
    let svg = &doc.resources["image2.svg"];
    assert_eq!(svg.role, ResourceRole::Alternate);
    assert_eq!(svg.companion_of.as_deref(), Some("image1.png"));
}

/// Build an XLSX containing one "Place in Cell" rich-value image in B1.
/// Mirrors the part layout Excel writes: cell `t="e" vm="1"` with a
/// `#VALUE!` placeholder, resolved through xl/metadata.xml and the
/// xl/richData/* chain to xl/media/image1.png.
fn create_rich_value_image_xlsx() -> Vec<u8> {
    create_test_zip(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata.xml"/>
  <Relationship Id="rId3" Type="http://schemas.microsoft.com/office/2017/06/relationships/rdRichValue" Target="richData/rdrichvalue.xml"/>
  <Relationship Id="rId4" Type="http://schemas.microsoft.com/office/2017/06/relationships/rdRichValueStructure" Target="richData/rdrichvaluestructure.xml"/>
  <Relationship Id="rId5" Type="http://schemas.microsoft.com/office/2022/10/relationships/richValueRel" Target="richData/richValueRel.xml"/>
</Relationships>"#,
        ),
        (
            "xl/worksheets/sheet1.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>label</t></is></c>
      <c r="B1" t="e" vm="1"><v>#VALUE!</v></c>
    </row>
  </sheetData>
</worksheet>"#,
        ),
        (
            "xl/metadata.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:xlrd="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata">
  <metadataTypes count="1">
    <metadataType name="XLRICHVALUE" minSupportedVersion="120000" copy="1" pasteAll="1" pasteValues="1" merge="1" splitFirst="1" rowColShift="1" clearFormats="1" clearComments="1" assign="1" coerce="1"/>
  </metadataTypes>
  <futureMetadata name="XLRICHVALUE" count="1">
    <bk><extLst><ext uri="{3e2802c4-a4d2-4d8b-9148-e3be6c30e623}"><xlrd:rvb i="0"/></ext></extLst></bk>
  </futureMetadata>
  <valueMetadata count="1">
    <bk><rc t="1" v="0"/></bk>
  </valueMetadata>
</metadata>"#,
        ),
        (
            "xl/richData/rdrichvaluestructure.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rvStructures xmlns="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata" count="1">
  <s t="_localImage">
    <k n="_rvRel:LocalImageIdentifier" t="i"/>
    <k n="CalcOrigin" t="i"/>
  </s>
</rvStructures>"#,
        ),
        (
            "xl/richData/rdrichvalue.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rvData xmlns="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata" count="1">
  <rv s="0"><v>0</v><v>5</v></rv>
</rvData>"#,
        ),
        (
            "xl/richData/richValueRel.xml",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<richValueRels xmlns="http://schemas.microsoft.com/office/spreadsheetml/2022/richvaluerel"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <rel r:id="rId1"/>
</richValueRels>"#,
        ),
        (
            "xl/richData/_rels/richValueRel.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
</Relationships>"#,
        ),
        ("xl/media/image1.png", "fake-png-bytes"),
    ])
}

#[test]
fn test_rich_value_in_cell_image_extracted() {
    let data = create_rich_value_image_xlsx();
    let mut parser = XlsxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let table = doc.sections[0]
        .content
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table missing");
    let cell = &table.rows[0].cells[1];

    assert!(
        !cell.plain_text().contains("#VALUE!"),
        "placeholder error text must be replaced by the image: {:?}",
        cell.plain_text()
    );
    let images: Vec<_> = cell.content.iter().flat_map(|p| &p.images).collect();
    assert_eq!(images.len(), 1, "in-cell image not extracted");
    assert_eq!(images[0].resource_id, "image1.png");

    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(
        md.contains("![") && md.contains("image1.png"),
        "markdown must reference the in-cell image:\n{md}"
    );
    assert!(
        doc.resources.contains_key("image1.png"),
        "media file must be extracted as a resource"
    );
}

#[test]
fn test_rich_value_vm_without_rich_data_parts_keeps_placeholder() {
    // A vm attribute with no resolvable richData chain (foreign producer,
    // stripped file) must degrade gracefully: keep the cell text, no panic.
    let workbook_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#;
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="e" vm="1"><v>#VALUE!</v></c>
    </row>
  </sheetData>
</worksheet>"#;
    let data = create_minimal_xlsx_with_parts(Some(workbook_rels), None, None, sheet_xml);

    let mut parser = XlsxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    let table = doc.sections[0]
        .content
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table missing");
    assert!(table.rows[0].cells[0].plain_text().contains("#VALUE!"));
}

#[test]
fn test_xlsx_requires_workbook_relationships() {
    let data = create_minimal_xlsx(None, None);
    let err = XlsxParser::from_bytes(data)
        .err()
        .expect("missing workbook relationships should fail");

    match err {
        Error::MissingComponent(path) => assert_eq!(path, "xl/_rels/workbook.xml.rels"),
        other => panic!("expected missing workbook rels error, got {other:?}"),
    }
}

#[test]
fn test_xlsx_rejects_malformed_workbook_relationships() {
    let data = create_minimal_xlsx(Some("<Relationships"), None);
    let err = XlsxParser::from_bytes(data)
        .err()
        .expect("malformed workbook relationships should fail");

    match err {
        Error::XmlParseWithContext { location, .. } => {
            assert_eq!(location, "xl/_rels/workbook.xml.rels")
        }
        other => panic!("expected malformed workbook rels error, got {other:?}"),
    }
}

#[test]
fn test_xlsx_non_utf8_workbook_is_error() {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/workbook.xml", options).unwrap();
    zip.write_all(b"<?xml version=\"1.0\"?><workbook>Caf\xe9</workbook>")
        .unwrap();

    let data = zip.finish().unwrap().into_inner();
    let err = match XlsxParser::from_bytes(data) {
        Ok(_) => panic!("non-UTF-8 workbook must surface Error::Encoding"),
        Err(err) => err,
    };

    assert!(
        matches!(err, Error::Encoding(_)),
        "expected Error::Encoding, got {err:?}"
    );
}

fn create_minimal_xlsx_with_malformed_optional_part(extra_part_path: &str) -> Vec<u8> {
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    let buf = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/workbook.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheets/>
</workbook>"#,
    )
    .unwrap();

    zip.start_file(extra_part_path, options).unwrap();
    zip.write_all(b"<?xml version=\"1.0\"?><root>Caf\xe9</root>")
        .unwrap();

    zip.finish().unwrap().into_inner()
}

#[test]
fn test_xlsx_non_utf8_optional_parts_surface_encoding_error() {
    // Optional XLSX parts (shared strings, styles) with malformed byte
    // content must surface Error::Encoding instead of being silently
    // treated as absent.
    for part_path in &["xl/sharedStrings.xml", "xl/styles.xml"] {
        let data = create_minimal_xlsx_with_malformed_optional_part(part_path);
        let err = match XlsxParser::from_bytes(data) {
            Ok(_) => panic!("malformed {part_path} must surface Error::Encoding"),
            Err(err) => err,
        };
        assert!(
            matches!(err, Error::Encoding(_)),
            "expected Error::Encoding for {part_path}, got {err:?}"
        );
    }
}

#[test]
fn test_xlsx_allows_missing_optional_sheet_relationships() {
    let data = create_minimal_xlsx(
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"></Relationship>
</Relationships>"#,
        ),
        None,
    );
    let mut parser = XlsxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    assert_eq!(doc.sections.len(), 1);
}

#[test]
fn test_xlsx_rejects_malformed_optional_sheet_relationships() {
    let data = create_minimal_xlsx(
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
        ),
        Some("<Relationships"),
    );
    let mut parser = XlsxParser::from_bytes(data).unwrap();
    let err = parser
        .parse()
        .expect_err("malformed optional sheet relationships should fail");

    match err {
        Error::XmlParseWithContext { location, .. } => {
            assert_eq!(location, "xl/worksheets/_rels/sheet1.xml.rels")
        }
        other => panic!("expected malformed optional sheet rels error, got {other:?}"),
    }
}

#[test]
fn test_xlsx_out_of_range_shared_string_index_is_error() {
    let data = create_minimal_xlsx_with_parts(
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
        ),
        None,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <si><t>Only entry</t></si>
</sst>"#,
        ),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="s"><v>3</v></c>
    </row>
  </sheetData>
</worksheet>"#,
    );
    let mut parser = XlsxParser::from_bytes(data).unwrap();
    let err = parser
        .parse()
        .expect_err("out-of-range shared string index should fail");

    match err {
        Error::InvalidData(message) => {
            assert!(message.contains("shared string index out of range: 3"))
        }
        other => panic!("expected invalid shared string error, got {other:?}"),
    }
}

#[test]
fn test_parse_drawing_images() {
    // Synthetic drawing XML with two images
    let drawing_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
                       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
                <xdr:twoCellAnchor>
                    <xdr:pic>
                        <xdr:nvPicPr>
                            <xdr:cNvPr id="2" name="Logo"/>
                            <xdr:cNvPicPr/>
                        </xdr:nvPicPr>
                        <xdr:blipFill>
                            <a:blip r:embed="rId1"/>
                        </xdr:blipFill>
                        <xdr:spPr>
                            <a:xfrm>
                                <a:off x="0" y="0"/>
                                <a:ext cx="914400" cy="457200"/>
                            </a:xfrm>
                        </xdr:spPr>
                    </xdr:pic>
                </xdr:twoCellAnchor>
                <xdr:twoCellAnchor>
                    <xdr:pic>
                        <xdr:nvPicPr>
                            <xdr:cNvPr id="3" name="Chart Screenshot"/>
                            <xdr:cNvPicPr/>
                        </xdr:nvPicPr>
                        <xdr:blipFill>
                            <a:blip r:embed="rId2"/>
                        </xdr:blipFill>
                        <xdr:spPr>
                            <a:xfrm>
                                <a:off x="100" y="100"/>
                                <a:ext cx="1828800" cy="1371600"/>
                            </a:xfrm>
                        </xdr:spPr>
                    </xdr:pic>
                </xdr:twoCellAnchor>
            </xdr:wsDr>"#;

    let mut rels = HashMap::new();
    rels.insert("rId1".to_string(), "image1.png".to_string());
    rels.insert("rId2".to_string(), "image2.jpeg".to_string());

    let images = XlsxParser::parse_drawing_images(drawing_xml, &rels).unwrap();
    assert_eq!(images.len(), 2);

    match &images[0] {
        Block::Image {
            resource_id,
            alt_text,
            width,
            height,
        } => {
            assert_eq!(resource_id, "image1.png");
            assert_eq!(alt_text.as_deref(), Some("Logo"));
            assert_eq!(*width, Some(914400));
            assert_eq!(*height, Some(457200));
        }
        other => panic!("Expected Block::Image, got {:?}", other),
    }

    match &images[1] {
        Block::Image {
            resource_id,
            alt_text,
            width,
            height,
        } => {
            assert_eq!(resource_id, "image2.jpeg");
            assert_eq!(alt_text.as_deref(), Some("Chart Screenshot"));
            assert_eq!(*width, Some(1828800));
            assert_eq!(*height, Some(1371600));
        }
        other => panic!("Expected Block::Image, got {:?}", other),
    }
}

/// A shape filled with a picture (`xdr:sp > xdr:spPr > a:blipFill`) shows that image on
/// the sheet like a picture does; a shape with a colour fill, or one whose text is all it
/// has, adds no image.
#[test]
fn test_parse_drawing_images_includes_picture_filled_shapes() {
    let drawing_xml = r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
                       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor>
    <xdr:sp macro="" textlink="">
      <xdr:nvSpPr><xdr:cNvPr id="2" name="Banner"/><xdr:cNvSpPr/></xdr:nvSpPr>
      <xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="500" cy="300"/></a:xfrm><a:blipFill><a:blip r:embed="rId1"/></a:blipFill></xdr:spPr>
      <xdr:txBody><a:p><a:r><a:t>Caption</a:t></a:r></a:p></xdr:txBody>
    </xdr:sp>
  </xdr:twoCellAnchor>
  <xdr:twoCellAnchor>
    <xdr:sp macro="" textlink="">
      <xdr:nvSpPr><xdr:cNvPr id="3" name="Plain"/><xdr:cNvSpPr/></xdr:nvSpPr>
      <xdr:spPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></xdr:spPr>
    </xdr:sp>
  </xdr:twoCellAnchor>
</xdr:wsDr>"#;
    let mut rels = HashMap::new();
    rels.insert("rId1".to_string(), "image1.png".to_string());

    let images = XlsxParser::parse_drawing_images(drawing_xml, &rels).unwrap();

    assert_eq!(images.len(), 1, "images: {images:?}");
    match &images[0] {
        Block::Image {
            resource_id,
            alt_text,
            width,
            height,
        } => {
            assert_eq!(resource_id, "image1.png");
            assert_eq!(alt_text.as_deref(), Some("Banner"));
            assert_eq!((*width, *height), (Some(500), Some(300)));
        }
        other => panic!("Expected Block::Image, got {other:?}"),
    }
}

/// A worksheet's background picture is `<picture r:id>`; a sheet without one has none.
#[test]
fn test_sheet_background_picture_relationship() {
    let with = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData/><picture r:id="rId3"/></worksheet>"#;
    let without = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#;
    assert_eq!(sheet_picture_rel(with).as_deref(), Some("rId3"));
    assert_eq!(sheet_picture_rel(without), None);
}

#[test]
fn test_parse_drawing_images_empty_blip() {
    // Test with self-closing blip element (empty element form)
    let drawing_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
                       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
                <xdr:oneCellAnchor>
                    <xdr:pic>
                        <xdr:nvPicPr>
                            <xdr:cNvPr id="2" name="Pic1"/>
                            <xdr:cNvPicPr/>
                        </xdr:nvPicPr>
                        <xdr:blipFill>
                            <a:blip r:embed="rId1"></a:blip>
                        </xdr:blipFill>
                        <xdr:spPr/>
                    </xdr:pic>
                </xdr:oneCellAnchor>
            </xdr:wsDr>"#;

    let mut rels = HashMap::new();
    rels.insert("rId1".to_string(), "image1.png".to_string());

    let images = XlsxParser::parse_drawing_images(drawing_xml, &rels).unwrap();
    assert_eq!(images.len(), 1);

    match &images[0] {
        Block::Image {
            resource_id,
            alt_text,
            ..
        } => {
            assert_eq!(resource_id, "image1.png");
            assert_eq!(alt_text.as_deref(), Some("Pic1"));
        }
        other => panic!("Expected Block::Image, got {:?}", other),
    }
}

#[test]
fn test_parse_drawing_no_images() {
    // Drawing with only shapes (no pic elements) — like existing test files
    let drawing_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
                       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
                <xdr:twoCellAnchor>
                    <xdr:sp macro="" textlink="">
                        <xdr:nvSpPr>
                            <xdr:cNvPr id="5" name="Rectangle 0"/>
                            <xdr:cNvSpPr/>
                        </xdr:nvSpPr>
                        <xdr:spPr>
                            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
                        </xdr:spPr>
                    </xdr:sp>
                </xdr:twoCellAnchor>
            </xdr:wsDr>"#;

    let rels = HashMap::new();
    let images = XlsxParser::parse_drawing_images(drawing_xml, &rels).unwrap();
    assert!(images.is_empty());
}

#[test]
fn test_parse_hyperlinks() {
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="s"><v>0</v></c>
                        <c r="B1" t="s"><v>1</v></c>
                    </row>
                </sheetData>
                <hyperlinks>
                    <hyperlink ref="A1" r:id="rId1"/>
                    <hyperlink ref="B1" r:id="rId2" display="Example"/>
                </hyperlinks>
            </worksheet>"#;

    let rels_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
                <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://rust-lang.org" TargetMode="External"/>
                <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
            </Relationships>"#;

    let sheet_rels = XlsxParser::parse_relationships(rels_xml);
    let hyperlinks = XlsxParser::parse_hyperlinks(sheet_xml, &sheet_rels);

    assert_eq!(hyperlinks.len(), 2);
    assert_eq!(hyperlinks.get("A1").unwrap(), "https://example.com");
    assert_eq!(hyperlinks.get("B1").unwrap(), "https://rust-lang.org");
}

#[test]
fn test_parse_hyperlinks_empty() {
    // Sheet with no hyperlinks section
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <sheetData>
                    <row r="1">
                        <c r="A1" t="s"><v>0</v></c>
                    </row>
                </sheetData>
            </worksheet>"#;

    let sheet_rels = HashMap::new();
    let hyperlinks = XlsxParser::parse_hyperlinks(sheet_xml, &sheet_rels);
    assert!(hyperlinks.is_empty());
}

#[test]
fn test_parse_hyperlinks_non_hyperlink_rels_ignored() {
    // hyperlink element references a non-hyperlink relationship (should be ignored)
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
                <sheetData/>
                <hyperlinks>
                    <hyperlink ref="A1" r:id="rId1"/>
                </hyperlinks>
            </worksheet>"#;

    let rels_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
            </Relationships>"#;

    let sheet_rels = XlsxParser::parse_relationships(rels_xml);
    let hyperlinks = XlsxParser::parse_hyperlinks(sheet_xml, &sheet_rels);
    assert!(hyperlinks.is_empty());
}

#[test]
fn test_parse_comments_xml_basic() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <authors>
                    <author>John Doe</author>
                </authors>
                <commentList>
                    <comment ref="A1" authorId="0">
                        <text>
                            <r><t>This is a comment</t></r>
                        </text>
                    </comment>
                    <comment ref="B5" authorId="0">
                        <text>
                            <r><t>Another comment</t></r>
                        </text>
                    </comment>
                </commentList>
            </comments>"#;

    let comments = XlsxParser::parse_comments_xml(xml);
    assert_eq!(comments.len(), 2);
    assert_eq!(comments.get("A1").unwrap(), "This is a comment");
    assert_eq!(comments.get("B5").unwrap(), "Another comment");
}

#[test]
fn test_parse_comments_xml_multiple_runs() {
    // Comment with multiple <r><t>...</t></r> runs should concatenate text
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <authors><author>Author</author></authors>
                <commentList>
                    <comment ref="C3" authorId="0">
                        <text>
                            <r><t>First part</t></r>
                            <r><t>Second part</t></r>
                        </text>
                    </comment>
                </commentList>
            </comments>"#;

    let comments = XlsxParser::parse_comments_xml(xml);
    assert_eq!(comments.len(), 1);
    assert_eq!(comments.get("C3").unwrap(), "First part Second part");
}

#[test]
fn test_parse_comments_xml_empty() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <authors><author>Author</author></authors>
                <commentList/>
            </comments>"#;

    let comments = XlsxParser::parse_comments_xml(xml);
    assert!(comments.is_empty());
}

#[test]
fn test_parse_comments_xml_case_insensitive_ref() {
    // Cell ref should be uppercased
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <authors><author>Author</author></authors>
                <commentList>
                    <comment ref="a1" authorId="0">
                        <text><r><t>Lowercase ref</t></r></text>
                    </comment>
                </commentList>
            </comments>"#;

    let comments = XlsxParser::parse_comments_xml(xml);
    assert_eq!(comments.len(), 1);
    assert!(comments.contains_key("A1"));
    assert_eq!(comments.get("A1").unwrap(), "Lowercase ref");
}

#[test]
fn test_parse_comments_xml_malformed_entity_preserves_raw_text() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
                <authors><author>Author</author></authors>
                <commentList>
                    <comment ref="A1" authorId="0">
                        <text><r><t>Bad &bogus; comment</t></r></text>
                    </comment>
                </commentList>
            </comments>"#;

    let comments = XlsxParser::parse_comments_xml(xml);
    assert_eq!(comments.get("A1").unwrap(), "Bad &bogus; comment");
}

#[test]
fn test_find_and_parse_comments_resolves_path() {
    // Verify find_and_parse_comments looks for "comments" in relationship types
    let rels_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
                <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
                <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/>
            </Relationships>"#;

    let sheet_rels = XlsxParser::parse_relationships(rels_xml);

    // Verify the relationship is correctly identified as comments type
    let (rel_type, target) = sheet_rels.get("rId2").unwrap();
    assert!(rel_type.contains("comments"));
    assert_eq!(target, "../comments1.xml");

    // Verify path resolution
    let resolved = XlsxParser::resolve_relative_path("xl/worksheets", target);
    assert_eq!(resolved, "xl/comments1.xml");
}

#[test]
fn test_xlsx_inline_str_mixed_entities_preserve_legitimate_and_malformed() {
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
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#).unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("xl/_rels/workbook.xml.rels", options)
            .unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("xl/workbook.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="S1" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
        )
        .unwrap();

        zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="str"><v>A &amp; B &bogus; C</v></c></row>
  </sheetData>
</worksheet>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let mut parser = XlsxParser::from_bytes(buf).expect("parser opens");
    let doc = parser.parse().expect("document parses");
    let text = doc.plain_text();
    assert!(
        text.contains("A & B &bogus; C"),
        "expected legitimate decoded + malformed preserved; got {text:?}"
    );
    assert!(
        !text.contains("A &amp; B"),
        "legitimate entity must not remain escaped; got {text:?}"
    );
}

#[test]
fn test_xlsx_missing_workbook_surfaces_missing_component() {
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
</Types>"#,
            )
            .unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
            )
            .unwrap();

        // xl/workbook.xml INTENTIONALLY ABSENT.
        // xl/_rels/workbook.xml.rels must be present — otherwise the earlier
        // read_required_relationships_for_part call in from_container would
        // fail first, masking the workbook-missing error we're testing.
        zip.start_file("xl/_rels/workbook.xml.rels", options)
            .unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let err = XlsxParser::from_bytes(buf)
        .err()
        .expect("must fail on missing workbook");
    match err {
        Error::MissingComponent(path) => {
            assert_eq!(path, "xl/workbook.xml");
        }
        other => panic!("expected MissingComponent(\"xl/workbook.xml\"), got {other:?}"),
    }
}
