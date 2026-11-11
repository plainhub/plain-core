use super::*;
use crate::error::Error;

fn empty_parser() -> PptxParser {
    use std::io::Cursor;
    let zip = plain_libs::zip::ZipWriter::new(Cursor::new(Vec::new()));
    let bytes = zip.finish().unwrap().into_inner();
    PptxParser {
        container: OoxmlContainer::from_bytes(bytes).unwrap(),
        slides: Vec::new(),
        relationships: HashMap::new(),
    }
}

#[test]
fn test_pptx_slide_text_entities_round_trip() {
    // Slide run text exercising the GeneralRef arm: predefined + numeric refs
    // decode, a mid-word "AT&amp;T" survives run fragmentation, and an unknown
    // &bogus; is preserved verbatim.
    let parser = empty_parser();
    let xml = r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree><p:sp><p:txBody>
    <a:p><a:r><a:t>AT&amp;T &lt;x&gt; &#48;&#x30; &bogus; end</a:t></a:r></a:p>
  </p:txBody></p:sp></p:spTree></p:cSld>
</p:sld>"#;

    let paragraphs = parser.parse_text_content(xml).unwrap();
    let text: String = paragraphs.iter().map(|p| p.plain_text()).collect();
    assert_eq!(text, "AT&T <x> 00 &bogus; end");
}

#[test]
fn test_pptx_stray_ampersand_does_not_abort() {
    // A raw `&` (ill-formed) must degrade gracefully rather than abort the slide.
    let parser = empty_parser();
    let xml = r#"<?xml version="1.0"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree><p:sp><p:txBody>
    <a:p><a:r><a:t>R&D dept</a:t></a:r></a:p>
  </p:txBody></p:sp></p:spTree></p:cSld>
</p:sld>"#;

    let paragraphs = parser
        .parse_text_content(xml)
        .expect("stray & must not abort");
    let text: String = paragraphs.iter().map(|p| p.plain_text()).collect();
    assert_eq!(text, "R&D dept");
}

/// Each `a:p` becomes a paragraph with its text, and run formatting survives: the
/// run marked `b="1"` is bold and the other is not.
#[test]
fn test_parse_text_content() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Body"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>Hello World</a:t></a:r></a:p><a:p><a:r><a:rPr b="1"/><a:t>Bold Text</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let data = deck(&[(&slide(shape), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let paras = paragraphs(&doc.sections[0]);
    assert_eq!(paras.len(), 2);
    assert_eq!(paras[0].plain_text(), "Hello World");
    assert!(!paras[0].runs[0].style.bold);
    assert_eq!(paras[1].plain_text(), "Bold Text");
    assert!(paras[1].runs[0].style.bold);
}

const EMPTY_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;

/// A deck of `slides` (each with its relationships part), plus arbitrary extra parts
/// such as media. The single-slide helpers below cannot express either.
fn deck(slides: &[(&str, &str)], extra_parts: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let mut zip = plain_libs::zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    let mut overrides = String::new();
    let mut presentation_rels = String::new();
    let mut slide_ids = String::new();
    for n in 1..=slides.len() {
        overrides.push_str(&format!(
                r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
            ));
        presentation_rels.push_str(&format!(
                r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#
            ));
        slide_ids.push_str(&format!(r#"<p:sldId id="{}" r:id="rId{n}"/>"#, 255 + n));
    }

    zip.start_file("[Content_Types].xml", options).unwrap();
    write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  {overrides}
</Types>"#
        )
        .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .unwrap();
    write!(
            zip,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{presentation_rels}</Relationships>"#
        )
        .unwrap();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    write!(
        zip,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>{slide_ids}</p:sldIdLst>
</p:presentation>"#
    )
    .unwrap();

    for (i, (slide_xml, slide_rels)) in slides.iter().enumerate() {
        let n = i + 1;
        zip.start_file(format!("ppt/slides/_rels/slide{n}.xml.rels"), options)
            .unwrap();
        zip.write_all(slide_rels.as_bytes()).unwrap();
        zip.start_file(format!("ppt/slides/slide{n}.xml"), options)
            .unwrap();
        zip.write_all(slide_xml.as_bytes()).unwrap();
    }

    for (path, data) in extra_parts {
        zip.start_file(*path, options).unwrap();
        zip.write_all(data).unwrap();
    }

    zip.finish().unwrap().into_inner()
}

fn slide(shapes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree>{shapes}</p:spTree></p:cSld>
</p:sld>"#
    )
}

/// A text shape; `placeholder` is the `<p:ph>` element, or empty for a plain box.
fn text_shape(placeholder: &str, text: &str) -> String {
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Shape"/><p:cNvSpPr/><p:nvPr>{placeholder}</p:nvPr></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp>"#
    )
}

fn paragraphs(section: &crate::model::Section) -> Vec<&crate::model::Paragraph> {
    section
        .content
        .iter()
        .filter_map(|block| match block {
            crate::model::Block::Paragraph(p) => Some(p),
            _ => None,
        })
        .collect()
}

/// Every slide becomes one section, in order, and the metadata's page count is the
/// number of slides.
#[test]
fn test_each_slide_becomes_a_section() {
    let first = slide(&text_shape("", "First slide"));
    let second = slide(&text_shape("", "Second slide"));
    let data = deck(&[(&first, EMPTY_RELS), (&second, EMPTY_RELS)], &[]);

    let mut parser = PptxParser::from_bytes(data).unwrap();
    assert_eq!(parser.slide_count(), 2);

    let doc = parser.parse().unwrap();
    assert_eq!(doc.sections.len(), 2);
    assert_eq!(doc.metadata.page_count, Some(2));
    assert_eq!(paragraphs(&doc.sections[0])[0].plain_text(), "First slide");
    assert_eq!(paragraphs(&doc.sections[1])[0].plain_text(), "Second slide");
}

/// Title and subtitle placeholders map to heading levels 1 and 2; a plain text box
/// stays body text.
#[test]
fn test_title_placeholders_become_headings() {
    use crate::model::HeadingLevel;

    let shapes = [
        text_shape(r#"<p:ph type="title"/>"#, "Deck Title"),
        text_shape(r#"<p:ph type="subTitle" idx="1"/>"#, "The Subtitle"),
        text_shape("", "Body text"),
    ]
    .concat();
    let data = deck(&[(&slide(&shapes), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let paras = paragraphs(&doc.sections[0]);
    let seen: Vec<(String, HeadingLevel)> =
        paras.iter().map(|p| (p.plain_text(), p.heading)).collect();
    assert_eq!(seen[0], ("Deck Title".to_string(), HeadingLevel::H1));
    assert_eq!(seen[1], ("The Subtitle".to_string(), HeadingLevel::H2));
    assert_eq!(seen[2].0, "Body text");
    assert!(!seen[2].1.is_heading());
}

/// A slide table becomes a table block with its cells in place, and renders as a
/// Markdown table.
#[test]
fn test_slide_table_becomes_a_table() {
    let cell = |text: &str| {
        format!(
            r#"<a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>{text}</a:t></a:r></a:p></a:txBody></a:tc>"#
        )
    };
    let table = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tr h="370840">{}{}</a:tr><a:tr h="370840">{}{}</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        cell("Name"),
        cell("Score"),
        cell("Ada"),
        cell("99")
    );
    let data = deck(&[(&slide(&table), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let tables: Vec<&crate::model::Table> = doc.sections[0]
        .content
        .iter()
        .filter_map(|block| match block {
            crate::model::Block::Table(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(tables.len(), 1);
    let texts: Vec<Vec<String>> = tables[0]
        .rows
        .iter()
        .map(|row| row.cells.iter().map(|c| c.plain_text()).collect())
        .collect();
    assert_eq!(texts, [["Name", "Score"], ["Ada", "99"]]);

    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(md.contains("| Name | Score |"), "markdown: {md}");
    assert!(md.contains("| Ada | 99 |"), "markdown: {md}");
}

/// A table cell's run keeps all of its formatting -- underline and strikethrough as well
/// as bold and italic -- as a run in a text box does.
#[test]
fn test_a_table_cell_run_keeps_its_underline_and_strikethrough() {
    let cell = |rpr: &str, text: &str| {
        format!(
            r#"<a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:rPr {rpr}/><a:t>{text}</a:t></a:r></a:p></a:txBody></a:tc>"#
        )
    };
    let table = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tr h="370840">{}{}</a:tr><a:tr h="370840">{}{}</a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        cell("", "Name"),
        cell("", "Note"),
        cell(r#"u="sng""#, "marked"),
        cell(r#"strike="sngStrike""#, "dropped")
    );
    let data = deck(&[(&slide(&table), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let runs: Vec<&TextRun> = doc.sections[0]
        .content
        .iter()
        .filter_map(|block| match block {
            crate::model::Block::Table(t) => Some(t),
            _ => None,
        })
        .flat_map(|t| t.rows.iter())
        .flat_map(|row| row.cells.iter())
        .flat_map(|cell| cell.content.iter())
        .flat_map(|p| p.runs.iter())
        .collect();
    let run = |text: &str| runs.iter().find(|r| r.text == text).copied().unwrap();
    assert!(run("marked").style.underline);
    assert!(run("dropped").style.strikethrough);
}

/// DrawingML writes every grid position as an `a:tc`, marking the ones a merge covers
/// with `hMerge`/`vMerge`. The merge is recorded on its owner, the covered positions
/// are not cells, and the table renders one column per grid column — including a row
/// that holds nothing but a covered position and an empty cell.
#[test]
fn test_slide_table_merges_are_recorded_on_their_owner() {
    let tc = |attrs: &str, text: &str| {
        format!(
            r#"<a:tc{attrs}><a:txBody><a:bodyPr/><a:p><a:r><a:t>{text}</a:t></a:r></a:p></a:txBody></a:tc>"#
        )
    };
    let empty_tc =
        |attrs: &str| format!(r#"<a:tc{attrs}><a:txBody><a:bodyPr/><a:p/></a:txBody></a:tc>"#);
    let rows = [
        // A header spanning columns 1-2, then column 3.
        format!(
            "{}{}{}",
            tc(r#" gridSpan="2""#, "Pair"),
            empty_tc(r#" hMerge="1""#),
            tc("", "C")
        ),
        // A label covering this row and the next.
        format!(
            "{}{}{}",
            tc(r#" rowSpan="2""#, "G"),
            tc("", "x"),
            tc("", "1")
        ),
        format!(
            "{}{}{}",
            empty_tc(r#" vMerge="1""#),
            empty_tc(""),
            empty_tc("")
        ),
        format!("{}{}{}", tc("", "H"), tc("", "z"), tc("", "3")),
    ];
    let body: String = rows
        .iter()
        .map(|r| format!(r#"<a:tr h="370840">{r}</a:tr>"#))
        .collect();
    let table = format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl>{body}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#
    );
    let data = deck(&[(&slide(&table), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let table = doc.sections[0]
        .content
        .iter()
        .find_map(|block| match block {
            crate::model::Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("a table");

    let texts: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| row.cells.iter().map(|c| c.plain_text()).collect())
        .collect();
    assert_eq!(
        texts,
        vec![
            vec!["Pair", "C"],
            vec!["G", "x", "1"],
            vec!["", ""],
            vec!["H", "z", "3"],
        ]
    );
    assert_eq!(table.rows[0].cells[0].col_span, 2);
    assert_eq!(table.rows[1].cells[0].row_span, 2);
    assert_eq!(table.cell_columns()[2], vec![1, 2]);

    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(md.contains("| H | z | 3 |"), "markdown: {md}");
    assert!(
        md.lines()
            .filter(|l| l.starts_with('|'))
            .all(|l| l.matches('|').count() == 4),
        "every table line must have three columns:\n{md}"
    );
}

/// A run's hyperlink resolves through the slide's own relationships to the external URL.
#[test]
fn test_hyperlink_resolves_through_the_slide_relationships() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Link"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:hlinkClick r:id="rIdLink"/></a:rPr><a:t>the spec</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/spec" TargetMode="External"/>
</Relationships>"#;
    let data = deck(&[(&slide(shape), rels)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let run = paragraphs(&doc.sections[0])
        .iter()
        .flat_map(|p| p.runs.iter())
        .find(|r| r.text == "the spec")
        .expect("the linked run");
    assert_eq!(run.hyperlink.as_deref(), Some("https://example.com/spec"));
}

/// A link that jumps to another slide of the deck leads to no address the output can
/// point at: its text is no link.
#[test]
fn test_a_jump_to_another_slide_is_no_link() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Link"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr><a:hlinkClick r:id="rIdSlide" action="ppaction://hlinksldjump"/></a:rPr><a:t>see slide 3</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSlide" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slide3.xml"/>
</Relationships>"#;
    let data = deck(&[(&slide(shape), rels)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let run = paragraphs(&doc.sections[0])
        .iter()
        .flat_map(|p| p.runs.iter())
        .find(|r| r.text == "see slide 3")
        .expect("the run");
    assert_eq!(run.hyperlink, None);
}

/// A linked run's underline is how the link looks, not the author's emphasis; an
/// underlined run outside a link keeps it.
#[test]
fn test_a_link_run_is_not_kept_underlined() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Link"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr u="sng"><a:hlinkClick r:id="rIdLink"/></a:rPr><a:t>the spec</a:t></a:r><a:r><a:rPr u="sng"/><a:t>marked</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/spec" TargetMode="External"/>
</Relationships>"#;
    let data = deck(&[(&slide(shape), rels)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let runs: Vec<&TextRun> = paragraphs(&doc.sections[0])
        .iter()
        .flat_map(|p| p.runs.iter())
        .collect();
    let link = runs.iter().find(|r| r.text == "the spec").unwrap();
    assert!(link.hyperlink.is_some() && !link.style.underline);
    let marked = runs.iter().find(|r| r.text == "marked").unwrap();
    assert!(marked.style.underline);
}

/// A line break inside a paragraph (`a:br`) ends the line of the run before it, and a
/// field (`a:fld` -- a slide number, a date) is read as a run of the value it last showed,
/// formatting and all; a field's own `a:pPr` is not the paragraph's.
#[test]
fn test_a_line_break_and_a_field_are_read_in_shape_text() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Body"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>First line</a:t></a:r><a:br><a:rPr lang="en-US"/></a:br><a:r><a:t>Second line</a:t></a:r></a:p><a:p><a:r><a:t>Slide </a:t></a:r><a:fld id="{6A1B}" type="slidenum"><a:rPr b="1"/><a:pPr lvl="3"/><a:t>7</a:t></a:fld></a:p></p:txBody></p:sp>"#;
    let data = deck(&[(&slide(shape), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let paras = paragraphs(&doc.sections[0]);
    assert_eq!(paras.len(), 2, "{paras:?}");
    let first: Vec<(&str, bool)> = paras[0]
        .runs
        .iter()
        .map(|r| (r.text.as_str(), r.line_break))
        .collect();
    assert_eq!(first, [("First line", true), ("Second line", false)]);
    let field = paras[1].runs.iter().find(|r| r.text == "7").unwrap();
    assert!(field.style.bold);
    assert_eq!(paras[1].plain_text(), "Slide 7");
}

/// A table cell's paragraphs read line breaks and fields as a text box's do.
#[test]
fn test_a_table_cell_reads_line_breaks_and_fields() {
    let table = r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tr h="370840"><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>Top</a:t></a:r><a:br/><a:r><a:t>Bottom</a:t></a:r></a:p></a:txBody></a:tc><a:tc><a:txBody><a:bodyPr/><a:p><a:fld id="{7C2D}" type="datetime1"><a:t>10/13/2026</a:t></a:fld></a:p></a:txBody></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#;
    let data = deck(&[(&slide(table), EMPTY_RELS)], &[]);
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let cells: Vec<&Cell> = doc.sections[0]
        .content
        .iter()
        .filter_map(|block| match block {
            crate::model::Block::Table(t) => Some(t),
            _ => None,
        })
        .flat_map(|t| t.rows.iter())
        .flat_map(|row| row.cells.iter())
        .collect();
    let first = &cells[0].content[0].runs;
    assert_eq!(first[0].text, "Top");
    assert!(first[0].line_break && !first[1].line_break);
    assert_eq!(cells[1].content[0].plain_text(), "10/13/2026");
}

/// Notes read fields too: a note's slide-number field is text, not nothing.
#[test]
fn test_notes_read_a_field() {
    let notes = r#"<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>See slide </a:t></a:r><a:fld id="{1}" type="slidenum"><a:t>3</a:t></a:fld></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#;
    let paras = empty_parser().parse_notes(notes).unwrap();
    assert_eq!(paras.len(), 1);
    assert_eq!(paras[0].plain_text(), "See slide 3");
}

/// The media a slide references is listed, typed by its extension; a media part no
/// slide references is not.
#[test]
fn test_extract_resources_lists_the_media_slides_reference() {
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
  <Relationship Id="rIdVid" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/video" Target="../media/clip.mp4"/>
</Relationships>"#;
    let data = deck(
        &[(&slide(&text_shape("", "x")), rels)],
        &[
            ("ppt/media/image1.png", b"\x89PNG\r\n\x1a\n"),
            ("ppt/media/clip.mp4", b"not really a video"),
            ("ppt/media/orphan.png", b"\x89PNG\r\n\x1a\n"),
        ],
    );
    let parser = PptxParser::from_bytes(data).unwrap();

    let mut resources = parser.extract_resources().unwrap();
    resources.sort_by(|a, b| a.filename.cmp(&b.filename));
    let names: Vec<_> = resources.iter().map(|r| r.filename.as_deref()).collect();
    assert_eq!(names, [Some("clip.mp4"), Some("image1.png")]);
    assert!(!resources[0].is_image());
    assert!(resources[1].is_image());
    assert_eq!(resources[1].data, b"\x89PNG\r\n\x1a\n");
}

/// A picture with an SVG original and an HD Photo layer is one picture: the raster image
/// is the primary, carrying the picture's description, and the other two are its
/// companions. The slide's image block names the primary, with the description — not
/// the shape's name — as its alt text.
#[test]
fn test_picture_companions_are_marked_and_alt_text_is_the_description() {
    let pic = r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="Picture 3" descr="Quarterly revenue"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rIdPng"><a:extLst><a:ext uri="{BEBA8EAE-BF5A-486C-A8C5-ECC9F3942E4B}"><a14:imgProps xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main"><a14:imgLayer r:embed="rIdWdp"/></a14:imgProps></a:ext><a:ext uri="{96DAC541-7B7A-43D3-8B79-37D633B846F1}"><asvg:svgBlip xmlns:asvg="http://schemas.microsoft.com/office/drawing/2016/SVG/main" r:embed="rIdSvg"/></a:ext></a:extLst></a:blip></p:blipFill><p:spPr/></p:pic>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdPng" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
  <Relationship Id="rIdSvg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image2.svg"/>
  <Relationship Id="rIdWdp" Type="http://schemas.microsoft.com/office/2007/relationships/hdphoto" Target="../media/hdphoto1.wdp"/>
</Relationships>"#;
    let data = deck(
        &[(&slide(pic), rels)],
        &[
            ("ppt/media/image1.png", b"\x89PNG\r\n\x1a\n"),
            ("ppt/media/image2.svg", b"<svg/>"),
            ("ppt/media/hdphoto1.wdp", b"II\xbc\x01"),
        ],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let png = &doc.resources["image1.png"];
    assert_eq!(png.role, ResourceRole::Primary);
    assert_eq!(png.alt_text.as_deref(), Some("Quarterly revenue"));
    let svg = &doc.resources["image2.svg"];
    assert_eq!(svg.role, ResourceRole::Alternate);
    assert_eq!(svg.companion_of.as_deref(), Some("image1.png"));
    let wdp = &doc.resources["hdphoto1.wdp"];
    assert_eq!(wdp.role, ResourceRole::Layer);
    assert_eq!(wdp.companion_of.as_deref(), Some("image1.png"));
    assert_eq!(wdp.mime_type.as_deref(), Some("image/vnd.ms-photo"));

    let images: Vec<_> = doc.sections[0]
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Image {
                resource_id,
                alt_text,
                ..
            } => Some((resource_id.as_str(), alt_text.as_deref())),
            _ => None,
        })
        .collect();
    assert_eq!(images, [("image1.png", Some("Quarterly revenue"))]);
}

/// Image rels for the picture-fill and picture-bullet tests below.
const IMAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdFill" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/fill.png"/>
  <Relationship Id="rIdBullet" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/bullet.png"/>
</Relationships>"#;

/// The image blocks of a section, as `(resource_id, alt_text)`.
fn image_blocks(section: &Section) -> Vec<(&str, Option<&str>)> {
    section
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Image {
                resource_id,
                alt_text,
                ..
            } => Some((resource_id.as_str(), alt_text.as_deref())),
            _ => None,
        })
        .collect()
}

/// A shape filled with a picture shows that picture: the slide references it as an
/// image, with the shape's description, and the shape's own text stays text. A shape with
/// a plain colour fill adds nothing.
#[test]
fn test_shape_picture_fill_is_an_image_of_the_slide() {
    let shapes = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Card" descr="Harbour at dusk"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="4000" cy="3000"/></a:xfrm><a:prstGeom prst="rect"/><a:blipFill><a:blip r:embed="rIdFill"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>Caption on the card</a:t></a:r></a:p></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="Plain"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></p:spPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>Plain shape</a:t></a:r></a:p></p:txBody></p:sp>"#;
    let data = deck(
        &[(&slide(shapes), IMAGE_RELS)],
        &[("ppt/media/fill.png", b"\x89PNG\r\n\x1a\n")],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let section = &doc.sections[0];
    assert_eq!(
        image_blocks(section),
        [("fill.png", Some("Harbour at dusk"))]
    );
    let texts: Vec<_> = paragraphs(section).iter().map(|p| p.plain_text()).collect();
    assert_eq!(texts, ["Caption on the card", "Plain shape"]);
    match section
        .content
        .iter()
        .find(|b| matches!(b, Block::Image { .. }))
    {
        Some(Block::Image { width, height, .. }) => {
            assert_eq!((*width, *height), (Some(4000), Some(3000)));
        }
        other => panic!("expected an image block, got {other:?}"),
    }
    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(
        md.contains("![Harbour at dusk](fill.png)"),
        "markdown: {md}"
    );
    assert_eq!(doc.resources["fill.png"].role, ResourceRole::Primary);
}

/// A picture bullet is a list marker, not a picture: the paragraph is a bullet item whose
/// `marker_image` names the image, the slide gets no image block per bullet, and the
/// Markdown stays a plain list. A paragraph that turns the bullet off is not a list item.
#[test]
fn test_picture_bullet_is_referenced_by_the_list_items() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Body"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/>
<a:p><a:pPr><a:buBlip><a:blip r:embed="rIdBullet"/></a:buBlip></a:pPr><a:r><a:t>First</a:t></a:r></a:p>
<a:p><a:pPr lvl="1"><a:buBlip><a:blip r:embed="rIdBullet"/></a:buBlip></a:pPr><a:r><a:t>Nested</a:t></a:r></a:p>
<a:p><a:pPr><a:buNone/></a:pPr><a:r><a:t>No bullet</a:t></a:r></a:p>
</p:txBody></p:sp>"#;
    let data = deck(
        &[(&slide(shape), IMAGE_RELS)],
        &[("ppt/media/bullet.png", b"\x89PNG\r\n\x1a\n")],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let section = &doc.sections[0];
    assert!(image_blocks(section).is_empty());
    let items: Vec<_> = paragraphs(section)
        .iter()
        .map(|p| {
            (
                p.plain_text(),
                p.list_info
                    .as_ref()
                    .map(|l| (l.list_type, l.level, l.marker_image.clone())),
            )
        })
        .collect();
    assert_eq!(
        items,
        [
            (
                "First".to_string(),
                Some((ListType::Bullet, 0, Some("bullet.png".to_string())))
            ),
            (
                "Nested".to_string(),
                Some((ListType::Bullet, 1, Some("bullet.png".to_string())))
            ),
            ("No bullet".to_string(), None),
        ]
    );
    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(
        md.contains("- First") && md.contains("  - Nested"),
        "markdown: {md}"
    );
    assert!(!md.contains("bullet.png"), "markdown: {md}");
    let json = plain_libs::serde_json::to_string(&doc).unwrap();
    assert!(json.contains("\"marker_image\":\"bullet.png\""), "{json}");
    assert_eq!(doc.resources["bullet.png"].role, ResourceRole::Primary);
    assert_eq!(doc.resources["bullet.png"].alt_text, None);
}

/// A picture bullet set once in a shape's list style applies to the paragraphs of that
/// level, unless a paragraph sets its own bullet.
#[test]
fn test_picture_bullet_from_the_shape_list_style() {
    let shape = r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Body"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/>
<a:lstStyle><a:lvl1pPr><a:buBlip><a:blip r:embed="rIdBullet"/></a:buBlip></a:lvl1pPr></a:lstStyle>
<a:p><a:r><a:t>Inherits</a:t></a:r></a:p>
<a:p><a:pPr><a:buChar char="-"/></a:pPr><a:r><a:t>Own bullet</a:t></a:r></a:p>
<a:p><a:pPr lvl="1"/><a:r><a:t>Other level</a:t></a:r></a:p>
</p:txBody></p:sp>"#;
    let data = deck(
        &[(&slide(shape), IMAGE_RELS)],
        &[("ppt/media/bullet.png", b"\x89PNG\r\n\x1a\n")],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    let markers: Vec<_> = paragraphs(&doc.sections[0])
        .iter()
        .map(|p| {
            p.list_info
                .as_ref()
                .and_then(|l| l.marker_image.as_deref().map(str::to_string))
        })
        .collect();
    assert_eq!(markers, [Some("bullet.png".to_string()), None, None]);
}

/// A picture fill or picture bullet on a slide layout is drawn by the layout, not the
/// slide: like every other layout media it is neither listed nor referenced.
#[test]
fn test_layout_picture_fill_and_bullet_are_not_slide_content() {
    let slide_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;
    let layout_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdFill" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/fill.png"/>
</Relationships>"#;
    let layout = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="Frame"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:blipFill><a:blip r:embed="rIdFill"/></a:blipFill></p:spPr></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="Body"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:p><a:pPr><a:buBlip><a:blip r:embed="rIdFill"/></a:buBlip></a:pPr><a:r><a:t>Prompt text</a:t></a:r></a:p></p:txBody></p:sp>
</p:spTree></p:cSld></p:sldLayout>"#;
    let data = deck(
        &[(&slide(&text_shape("", "body")), slide_rels)],
        &[
            ("ppt/slideLayouts/slideLayout1.xml", layout.as_bytes()),
            (
                "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
                layout_rels.as_bytes(),
            ),
            ("ppt/media/fill.png", b"\x89PNG\r\n\x1a\n"),
        ],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    assert!(
        doc.resources.is_empty(),
        "resources: {:?}",
        doc.resources.keys()
    );
    assert!(image_blocks(&doc.sections[0]).is_empty());
}

const IMAGE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const LAYOUT_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const MASTER_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";

fn rels_xml(entries: &[(&str, &str, &str)]) -> String {
    let body: String = entries
        .iter()
        .map(|(id, ty, target)| {
            format!(r#"<Relationship Id="{id}" Type="{ty}" Target="{target}"/>"#)
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A slide whose own background is a picture references it from the slide (nothing in
/// the text); a colour background references nothing. The background a layout gives
/// the slide is template decoration, like the rest of the layout's media.
#[test]
fn test_slide_background_picture_is_referenced_by_the_section() {
    let with_bg = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:cSld><p:bg><p:bgPr><a:blipFill dpi="0"><a:blip r:embed="rIdBg"/><a:stretch><a:fillRect/></a:stretch></a:blipFill><a:effectLst/></p:bgPr></p:bg><p:spTree></p:spTree></p:cSld></p:sld>"#;
    let colour_bg = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></p:bgPr></p:bg><p:spTree></p:spTree></p:cSld></p:sld>"#;
    let with_rels = rels_xml(&[
        ("rIdBg", IMAGE_REL, "../media/bg.png"),
        ("rIdLayout", LAYOUT_REL, "../slideLayouts/slideLayout1.xml"),
    ]);
    let layout_rels = rels_xml(&[("rIdLb", IMAGE_REL, "../media/layoutbg.png")]);
    let layout = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:bg><p:bgPr><a:blipFill><a:blip r:embed="rIdLb"/></a:blipFill></p:bgPr></p:bg><p:spTree/></p:cSld></p:sldLayout>"#;
    let colour_rels = rels_xml(&[("rIdLayout", LAYOUT_REL, "../slideLayouts/slideLayout1.xml")]);
    let data = deck(
        &[(with_bg, &with_rels), (colour_bg, &colour_rels)],
        &[
            ("ppt/slideLayouts/slideLayout1.xml", layout.as_bytes()),
            (
                "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
                layout_rels.as_bytes(),
            ),
            ("ppt/media/bg.png", PNG),
            ("ppt/media/layoutbg.png", PNG),
        ],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();

    assert_eq!(doc.sections[0].background_image.as_deref(), Some("bg.png"));
    assert_eq!(doc.sections[1].background_image, None);
    assert!(
        doc.sections[0].content.is_empty(),
        "a background is no block"
    );
    assert!(doc.resources.contains_key("bg.png"));
    assert!(!doc.resources.contains_key("layoutbg.png"));
    let json = plain_libs::serde_json::to_string(&doc.sections[0]).unwrap();
    assert!(json.contains("\"background_image\":\"bg.png\""), "{json}");
    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(!md.contains("bg.png"), "markdown: {md}");
}

/// A table cell filled with a picture references it from the cell; the cell's text
/// stays its text and a picture bullet in a cell paragraph names its image.
#[test]
fn test_table_cell_picture_fill_and_bullet_are_referenced() {
    let table = r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="Table"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><a:graphic><a:graphicData><a:tbl><a:tblGrid><a:gridCol w="100"/><a:gridCol w="100"/></a:tblGrid>
<a:tr h="10"><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>Filled</a:t></a:r></a:p></a:txBody><a:tcPr><a:blipFill><a:blip r:embed="rIdCell"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></a:tcPr></a:tc><a:tc><a:txBody><a:bodyPr/><a:p><a:pPr><a:buBlip><a:blip r:embed="rIdBullet"/></a:buBlip></a:pPr><a:r><a:t>Bulleted</a:t></a:r></a:p></a:txBody><a:tcPr><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></a:tcPr></a:tc></a:tr>
<a:tr h="10"><a:tc><a:txBody><a:bodyPr/><a:p><a:endParaRPr/></a:p></a:txBody><a:tcPr><a:blipFill><a:blip r:embed="rIdCell"/></a:blipFill></a:tcPr></a:tc><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>Plain</a:t></a:r></a:p></a:txBody></a:tc></a:tr>
</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#;
    let rels = rels_xml(&[
        ("rIdCell", IMAGE_REL, "../media/cell.png"),
        ("rIdBullet", IMAGE_REL, "../media/bullet.png"),
    ]);
    let data = deck(
        &[(&slide(table), &rels)],
        &[("ppt/media/cell.png", PNG), ("ppt/media/bullet.png", PNG)],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let Block::Table(t) = &doc.sections[0].content[0] else {
        panic!("expected a table: {:?}", doc.sections[0].content);
    };
    let cell = |r: usize, c: usize| &t.rows[r].cells[c];
    assert_eq!(cell(0, 0).background_image.as_deref(), Some("cell.png"));
    assert_eq!(cell(0, 0).plain_text(), "Filled");
    assert_eq!(cell(0, 1).background_image, None);
    assert_eq!(cell(1, 0).background_image.as_deref(), Some("cell.png"));
    assert_eq!(cell(1, 1).background_image, None);
    let bullet = cell(0, 1).content[0].list_info.as_ref().unwrap();
    assert_eq!(bullet.marker_image.as_deref(), Some("bullet.png"));
    assert!(doc.resources.contains_key("cell.png"));
    assert!(doc.resources.contains_key("bullet.png"));
    assert!(image_blocks(&doc.sections[0]).is_empty());
    let md = crate::render::to_markdown(&doc, &crate::render::RenderOptions::default()).unwrap();
    assert!(!md.contains(".png"), "markdown: {md}");
}

/// Picture bullets come down the placeholder chain — master `txStyles`, master
/// placeholder, layout placeholder, slide shape, paragraph — level by level; a bullet
/// of another kind replaces an inherited picture. The image a slide inherits is listed
/// with the slide's media; one no slide paragraph ends up with is not.
#[test]
fn test_picture_bullets_are_inherited_through_the_placeholder_chain() {
    let master = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree/></p:cSld>
<p:txStyles><p:titleStyle><a:lvl1pPr><a:buNone/></a:lvl1pPr></p:titleStyle>
<p:bodyStyle><a:lvl1pPr><a:buBlip><a:blip r:embed="rIdMasterBullet"/></a:buBlip></a:lvl1pPr><a:lvl2pPr><a:buBlip><a:blip r:embed="rIdMasterBullet"/></a:buBlip></a:lvl2pPr></p:bodyStyle>
<p:otherStyle><a:lvl1pPr><a:buBlip><a:blip r:embed="rIdUnused"/></a:buBlip></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#;
    let master_rels = rels_xml(&[
        ("rIdMasterBullet", IMAGE_REL, "../media/master-bullet.png"),
        ("rIdUnused", IMAGE_REL, "../media/unused.png"),
    ]);
    // Layout: idx 1 sets its own level-1 picture and replaces level 2 with a character
    // bullet; idx 2 sets nothing and takes the master's.
    let layout = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="A"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle><a:lvl1pPr><a:buBlip><a:blip r:embed="rIdLayoutBullet"/></a:buBlip></a:lvl1pPr><a:lvl2pPr><a:buChar char="-"/></a:lvl2pPr></a:lstStyle><a:p><a:r><a:t>Prompt</a:t></a:r></a:p></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="B"/><p:cNvSpPr/><p:nvPr><p:ph idx="2"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Prompt</a:t></a:r></a:p></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="4" name="T"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Prompt</a:t></a:r></a:p></p:txBody></p:sp>
</p:spTree></p:cSld></p:sldLayout>"#;
    let layout_rels = rels_xml(&[
        ("rIdLayoutBullet", IMAGE_REL, "../media/layout-bullet.png"),
        ("rIdMaster", MASTER_REL, "../slideMasters/slideMaster1.xml"),
    ]);
    let ph = |ph: &str, paras: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="9" name="S"/><p:cNvSpPr/><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paras}</p:txBody></p:sp>"#
        )
    };
    let p = |ppr: &str, text: &str| {
        format!(r#"<a:p><a:pPr{ppr}</a:pPr><a:r><a:t>{text}</a:t></a:r></a:p>"#)
    };
    let shapes = [
        ph(
            r#"<p:ph type="body" idx="1"/>"#,
            &[
                p(">", "layout one"),
                p(r#" lvl="1">"#, "layout char overrides master"),
                p("><a:buNone/>", "no bullet"),
            ]
            .concat(),
        ),
        ph(
            r#"<p:ph idx="2"/>"#,
            &[p(">", "master one"), p(r#" lvl="1">"#, "master two")].concat(),
        ),
        ph(r#"<p:ph type="title"/>"#, &p(">", "A title")),
        // A plain text box inherits nothing.
        ph("", &p(">", "text box")),
    ]
    .concat();
    let slide_rels = rels_xml(&[("rIdLayout", LAYOUT_REL, "../slideLayouts/slideLayout1.xml")]);
    let data = deck(
        &[(&slide(&shapes), &slide_rels)],
        &[
            ("ppt/slideLayouts/slideLayout1.xml", layout.as_bytes()),
            (
                "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
                layout_rels.as_bytes(),
            ),
            ("ppt/slideMasters/slideMaster1.xml", master.as_bytes()),
            (
                "ppt/slideMasters/_rels/slideMaster1.xml.rels",
                master_rels.as_bytes(),
            ),
            ("ppt/media/layout-bullet.png", PNG),
            ("ppt/media/master-bullet.png", PNG),
            ("ppt/media/unused.png", PNG),
        ],
    );
    let doc = PptxParser::from_bytes(data).unwrap().parse().unwrap();
    let got: Vec<(String, Option<(u8, String)>)> = paragraphs(&doc.sections[0])
        .iter()
        .map(|para| {
            (
                para.plain_text(),
                para.list_info
                    .as_ref()
                    .map(|l| (l.level, l.marker_image.clone().unwrap_or_default())),
            )
        })
        .collect();
    let some = |level: u8, img: &str| Some((level, img.to_string()));
    assert_eq!(
        got,
        [
            ("layout one".to_string(), some(0, "layout-bullet.png")),
            ("layout char overrides master".to_string(), None),
            ("no bullet".to_string(), None),
            ("master one".to_string(), some(0, "master-bullet.png")),
            ("master two".to_string(), some(1, "master-bullet.png")),
            ("A title".to_string(), None),
            ("text box".to_string(), None),
        ]
    );
    assert!(doc.resources.contains_key("layout-bullet.png"));
    assert!(doc.resources.contains_key("master-bullet.png"));
    assert!(
        !doc.resources.contains_key("unused.png"),
        "resources: {:?}",
        doc.resources.keys()
    );
}

/// A one-slide deck whose slide points at `slideLayout1.xml`, with the layout's own
/// relationships part holding `layout_rels`.
fn deck_with_layout(layout_rels: &[u8]) -> Vec<u8> {
    let slide_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#;
    let layout: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#;
    deck(
        &[(&slide(&text_shape("", "body")), slide_rels)],
        &[
            ("ppt/slideLayouts/slideLayout1.xml", layout),
            ("ppt/slideLayouts/_rels/slideLayout1.xml.rels", layout_rels),
        ],
    )
}

/// Control: with readable layout relationships the deck parses, so the test below is
/// about the relationships part and not about the layout path being skipped.
#[test]
fn test_deck_with_a_layout_parses() {
    let doc = PptxParser::from_bytes(deck_with_layout(EMPTY_RELS.as_bytes()))
        .unwrap()
        .parse()
        .unwrap();

    assert_eq!(paragraphs(&doc.sections[0])[0].plain_text(), "body");
}

/// A layout's relationships part is optional, but one that is present and unreadable is
/// damage, not absence -- the contract every other optional part in this crate keeps.
/// It used to be read as "no relationships", which silently dropped inherited
/// placeholders.
#[test]
fn test_malformed_layout_relationships_are_reported() {
    let data = deck_with_layout(b"<Relationships>caf\xe9</Relationships>");

    let err = PptxParser::from_bytes(data).unwrap().parse().unwrap_err();

    assert!(matches!(err, Error::Encoding(_)), "got {err:?}");
}

/// Helper to create a minimal PPTX in memory with given slide XML content.
fn create_minimal_pptx(slide_xml: &str) -> Vec<u8> {
    create_minimal_pptx_with_relationships(
        slide_xml,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#,
        ),
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
        ),
    )
}

fn create_minimal_pptx_with_notes(slide_xml: &str, notes_xml: &str) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/notesSlides/notesSlide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId1"/>
  </p:sldIdLst>
</p:presentation>"#,
    )
    .unwrap();

    zip.start_file("ppt/slides/_rels/slide1.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("ppt/slides/slide1.xml", options).unwrap();
    zip.write_all(slide_xml.as_bytes()).unwrap();

    zip.start_file("ppt/notesSlides/notesSlide1.xml", options)
        .unwrap();
    zip.write_all(notes_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

fn create_minimal_pptx_with_relationships(
    slide_xml: &str,
    presentation_rels_xml: Option<&str>,
    slide_rels_xml: Option<&str>,
) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#).unwrap();

    // _rels/.rels
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    if let Some(presentation_rels_xml) = presentation_rels_xml {
        // ppt/_rels/presentation.xml.rels
        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        zip.write_all(presentation_rels_xml.as_bytes()).unwrap();
    }

    // ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId1"/>
  </p:sldIdLst>
</p:presentation>"#,
    )
    .unwrap();

    if let Some(slide_rels_xml) = slide_rels_xml {
        // ppt/slides/_rels/slide1.xml.rels
        zip.start_file("ppt/slides/_rels/slide1.xml.rels", options)
            .unwrap();
        zip.write_all(slide_rels_xml.as_bytes()).unwrap();
    }

    // ppt/slides/slide1.xml
    zip.start_file("ppt/slides/slide1.xml", options).unwrap();
    zip.write_all(slide_xml.as_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
}

#[test]
fn test_pptx_chart_invalid_numeric_value_propagates_error() {
    use std::io::{Cursor, Write};
    use plain_libs::zip::write::SimpleFileOptions;

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

    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <p:cSld><p:spTree>
    <p:graphicFrame><a:graphic><a:graphicData>
      <c:chart r:id="rIdChart"/>
    </a:graphicData></a:graphic></p:graphicFrame>
  </p:spTree></p:cSld>
</p:sld>"#;

    let slide_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
</Relationships>"#;

    let presentation_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rIdSlide"/></p:sldIdLst>
</p:presentation>"#;

    let presentation_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSlide" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#;

    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(presentation_xml.as_bytes()).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .unwrap();
    zip.write_all(presentation_rels.as_bytes()).unwrap();

    zip.start_file("ppt/slides/slide1.xml", options).unwrap();
    zip.write_all(slide_xml.as_bytes()).unwrap();

    zip.start_file("ppt/slides/_rels/slide1.xml.rels", options)
        .unwrap();
    zip.write_all(slide_rels.as_bytes()).unwrap();

    zip.start_file("ppt/charts/chart1.xml", options).unwrap();
    zip.write_all(chart_xml.as_bytes()).unwrap();

    let data = zip.finish().unwrap().into_inner();
    let mut parser = PptxParser::from_bytes(data).unwrap();
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
fn test_pptx_missing_chart_part_propagates_error() {
    use std::io::{Cursor, Write};
    use plain_libs::zip::write::SimpleFileOptions;

    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <p:cSld><p:spTree>
    <p:graphicFrame><a:graphic><a:graphicData>
      <c:chart r:id="rIdChart"/>
    </a:graphicData></a:graphic></p:graphicFrame>
  </p:spTree></p:cSld>
</p:sld>"#;

    let slide_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
</Relationships>"#;

    let presentation_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rIdSlide"/></p:sldIdLst>
</p:presentation>"#;

    let presentation_rels = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSlide" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#;

    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(presentation_xml.as_bytes()).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .unwrap();
    zip.write_all(presentation_rels.as_bytes()).unwrap();

    zip.start_file("ppt/slides/slide1.xml", options).unwrap();
    zip.write_all(slide_xml.as_bytes()).unwrap();

    zip.start_file("ppt/slides/_rels/slide1.xml.rels", options)
        .unwrap();
    zip.write_all(slide_rels.as_bytes()).unwrap();

    let data = zip.finish().unwrap().into_inner();
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let err = parser
        .parse()
        .expect_err("missing referenced chart part must surface");

    match err {
        Error::MissingComponent(path) => assert_eq!(path, "ppt/charts/chart1.xml"),
        other => panic!("expected MissingComponent, got {other:?}"),
    }
}

#[test]
fn test_pptx_slide_table_preserves_raw_malformed_entity() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:graphicFrame><a:graphic><a:graphicData>
      <a:tbl>
        <a:tr><a:tc><a:txBody><a:p><a:r><a:t>Cell &bogus; text</a:t></a:r></a:p></a:txBody></a:tc></a:tr>
      </a:tbl>
    </a:graphicData></a:graphic></p:graphicFrame>
  </p:spTree></p:cSld>
</p:sld>"#;

    let data = create_minimal_pptx(slide_xml);
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    assert!(
        doc.plain_text().contains("Cell &bogus; text"),
        "expected raw malformed entity preserved in slide table, got: {}",
        doc.plain_text()
    );
}

#[test]
fn test_pptx_slide_text_preserves_raw_malformed_entity() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:sp><p:txBody>
      <a:p><a:r><a:t>Slide &bogus; body</a:t></a:r></a:p>
    </p:txBody></p:sp>
  </p:spTree></p:cSld>
</p:sld>"#;

    let data = create_minimal_pptx(slide_xml);
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    assert!(
        doc.plain_text().contains("Slide &bogus; body"),
        "expected raw malformed entity preserved in slide text, got: {}",
        doc.plain_text()
    );
}

#[test]
fn test_pptx_notes_preserves_raw_malformed_entity() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree/></p:cSld>
</p:sld>"#;

    let notes_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
         xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:sp><p:txBody>
      <a:p><a:r><a:t>Note &bogus; body</a:t></a:r></a:p>
    </p:txBody></p:sp>
  </p:spTree></p:cSld>
</p:notes>"#;

    let data = create_minimal_pptx_with_notes(slide_xml, notes_xml);
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let notes = doc.sections[0]
        .notes
        .as_ref()
        .expect("notes should be parsed");
    let notes_text = notes
        .iter()
        .map(Paragraph::plain_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        notes_text.contains("Note &bogus; body"),
        "expected raw malformed entity preserved in notes, got: {}",
        notes_text
    );
}

#[test]
fn test_pptx_requires_presentation_relationships() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#;

    let data = create_minimal_pptx_with_relationships(slide_xml, None, None);
    let err = PptxParser::from_bytes(data)
        .err()
        .expect("missing presentation relationships should fail");

    match err {
        Error::MissingComponent(path) => assert_eq!(path, "ppt/_rels/presentation.xml.rels"),
        other => panic!("expected missing presentation rels error, got {other:?}"),
    }
}

#[test]
fn test_pptx_rejects_malformed_presentation_relationships() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#;

    let data = create_minimal_pptx_with_relationships(slide_xml, Some("<Relationships"), None);
    let err = PptxParser::from_bytes(data)
        .err()
        .expect("malformed presentation relationships should fail");

    match err {
        Error::XmlParseWithContext { location, .. } => {
            assert_eq!(location, "ppt/_rels/presentation.xml.rels")
        }
        other => panic!("expected malformed presentation rels error, got {other:?}"),
    }
}

#[test]
fn test_pptx_non_utf8_presentation_is_error() {
    use std::io::{Cursor, Write};
    use plain_libs::zip::write::SimpleFileOptions;

    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(b"<?xml version=\"1.0\"?><presentation>Caf\xe9</presentation>")
        .unwrap();

    let data = zip.finish().unwrap().into_inner();
    let err = match PptxParser::from_bytes(data) {
        Ok(_) => panic!("non-UTF-8 presentation must surface Error::Encoding"),
        Err(err) => err,
    };
    assert!(
        matches!(err, Error::Encoding(_)),
        "expected Error::Encoding, got {err:?}"
    );
}

fn create_minimal_pptx_with_malformed_part(malformed_part_path: &str) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let buf = Cursor::new(Vec::new());
    let mut zip = plain_libs::zip::ZipWriter::new(buf);
    let options =
        plain_libs::zip::write::SimpleFileOptions::default().compression_method(plain_libs::zip::CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
  <Override PartName="/ppt/notesSlides/notesSlide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"/>
</Types>"#).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/_rels/presentation.xml.rels", options)
        .unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#).unwrap();

    zip.start_file("ppt/presentation.xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>
    <p:sldId id="256" r:id="rId1"/>
  </p:sldIdLst>
</p:presentation>"#,
    )
    .unwrap();

    zip.start_file("ppt/slides/_rels/slide1.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    let valid_slide = br#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree/></p:cSld>
</p:sld>"#;
    let valid_notes = br#"<?xml version="1.0" encoding="UTF-8"?>
<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
         xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree/></p:cSld>
</p:notes>"#;
    let malformed = b"<?xml version=\"1.0\"?><root>Caf\xe9</root>";

    zip.start_file("ppt/slides/slide1.xml", options).unwrap();
    if malformed_part_path == "ppt/slides/slide1.xml" {
        zip.write_all(malformed).unwrap();
    } else {
        zip.write_all(valid_slide).unwrap();
    }

    zip.start_file("ppt/notesSlides/notesSlide1.xml", options)
        .unwrap();
    if malformed_part_path == "ppt/notesSlides/notesSlide1.xml" {
        zip.write_all(malformed).unwrap();
    } else {
        zip.write_all(valid_notes).unwrap();
    }

    zip.finish().unwrap().into_inner()
}

#[test]
fn test_pptx_non_utf8_optional_parts_surface_encoding_error() {
    // Slide and notes parts with malformed (non-UTF-8/UTF-16) byte content
    // must surface Error::Encoding instead of being silently dropped.
    for part_path in &["ppt/slides/slide1.xml", "ppt/notesSlides/notesSlide1.xml"] {
        let data = create_minimal_pptx_with_malformed_part(part_path);
        let mut parser = PptxParser::from_bytes(data).expect("constructor must succeed");
        let err = match parser.parse() {
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
fn test_pptx_allows_missing_optional_slide_relationships() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree/></p:cSld>
</p:sld>"#;

    let data = create_minimal_pptx_with_relationships(
        slide_xml,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"></Relationship>
</Relationships>"#,
        ),
        None,
    );
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();

    assert_eq!(doc.sections.len(), 1);
}

#[test]
fn test_pptx_rejects_malformed_optional_slide_relationships() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Hello from slide</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

    let data = create_minimal_pptx_with_relationships(
        slide_xml,
        Some(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#,
        ),
        Some("<Relationships"),
    );
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let err = parser
        .parse()
        .expect_err("malformed optional slide relationships should fail");

    match err {
        Error::XmlParseWithContext { location, .. } => {
            assert_eq!(location, "ppt/slides/_rels/slide1.xml.rels")
        }
        other => panic!("expected malformed optional slide rels error, got {other:?}"),
    }
}

#[test]
fn test_parse_grouped_shapes() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:txBody>
          <a:p><a:r><a:t>Top level shape</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:grpSp>
        <p:nvGrpSpPr>
          <p:cNvPr id="10" name="Group 1"/>
          <p:cNvGrpSpPr/>
          <p:nvPr/>
        </p:nvGrpSpPr>
        <p:grpSpPr/>
        <p:sp>
          <p:txBody>
            <a:p><a:r><a:t>Grouped shape 1</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>
        <p:sp>
          <p:txBody>
            <a:p><a:r><a:t>Grouped shape 2</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>
      </p:grpSp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

    let data = create_minimal_pptx(slide_xml);
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = doc.plain_text();

    assert!(
        text.contains("Top level shape"),
        "Should contain top-level shape text, got: {}",
        text
    );
    assert!(
        text.contains("Grouped shape 1"),
        "Should contain first grouped shape text, got: {}",
        text
    );
    assert!(
        text.contains("Grouped shape 2"),
        "Should contain second grouped shape text, got: {}",
        text
    );
}

#[test]
fn test_parse_nested_grouped_shapes() {
    let slide_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:grpSp>
        <p:nvGrpSpPr><p:cNvPr id="10" name="Outer Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
        <p:grpSpPr/>
        <p:sp>
          <p:txBody>
            <a:p><a:r><a:t>Outer group text</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>
        <p:grpSp>
          <p:nvGrpSpPr><p:cNvPr id="20" name="Inner Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
          <p:grpSpPr/>
          <p:sp>
            <p:txBody>
              <a:p><a:r><a:t>Inner group text</a:t></a:r></a:p>
            </p:txBody>
          </p:sp>
        </p:grpSp>
      </p:grpSp>
    </p:spTree>
  </p:cSld>
</p:sld>"#;

    let data = create_minimal_pptx(slide_xml);
    let mut parser = PptxParser::from_bytes(data).unwrap();
    let doc = parser.parse().unwrap();
    let text = doc.plain_text();

    assert!(
        text.contains("Outer group text"),
        "Should contain outer group shape text, got: {}",
        text
    );
    assert!(
        text.contains("Inner group text"),
        "Should contain inner (nested) group shape text, got: {}",
        text
    );
}

#[test]
fn test_pptx_slide_mixed_entities_preserve_legitimate_and_malformed() {
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = plain_libs::zip::ZipWriter::new(cursor);
        let options = plain_libs::zip::write::SimpleFileOptions::default()
            .compression_method(plain_libs::zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#).unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#).unwrap();

        zip.start_file("ppt/presentation.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst>
</p:presentation>"#,
        )
        .unwrap();

        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:sp><p:txBody><a:p><a:r><a:t>A &amp; B &bogus; C</a:t></a:r></a:p></p:txBody></p:sp>
  </p:spTree></p:cSld>
</p:sld>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let mut parser = PptxParser::from_bytes(buf).expect("parser opens");
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
fn test_pptx_missing_presentation_surfaces_missing_component() {
    use std::io::Write;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = plain_libs::zip::ZipWriter::new(cursor);
        let options = plain_libs::zip::write::SimpleFileOptions::default()
            .compression_method(plain_libs::zip::CompressionMethod::Stored);

        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
</Types>"#,
            )
            .unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#,
            )
            .unwrap();

        // ppt/presentation.xml INTENTIONALLY ABSENT.
        // Empty ppt/_rels/presentation.xml.rels so the required rels
        // read succeeds and we reach the presentation-missing path.
        zip.start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let err = PptxParser::from_bytes(buf)
        .err()
        .expect("must fail on missing presentation");
    match err {
        Error::MissingComponent(path) => {
            assert_eq!(path, "ppt/presentation.xml");
        }
        other => panic!("expected MissingComponent(\"ppt/presentation.xml\"), got {other:?}"),
    }
}
