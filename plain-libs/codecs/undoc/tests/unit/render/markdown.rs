use super::*;
use crate::detect::FormatType;
use crate::model::{Cell, HeadingLevel, RevisionType, Row, Section, TextStyle};
use crate::render::options::SectionMarkerStyle;

fn two_section_doc(format: FormatType, names: [&str; 2]) -> Document {
    let mut doc = Document::new();
    doc.format = format;
    let mut s0 = Section::new(0);
    s0.name = Some(names[0].to_string());
    let mut s1 = Section::new(1);
    s1.name = Some(names[1].to_string());
    doc.sections.push(s0);
    doc.sections.push(s1);
    doc
}

#[test]
fn test_pptx_section_markers_comment() {
    let doc = two_section_doc(FormatType::Pptx, ["Introduction", "Conclusion"]);
    let opts = RenderOptions::new().with_section_markers(SectionMarkerStyle::Comment);
    let md = to_markdown(&doc, &opts).unwrap();
    assert!(
        md.contains("<!-- slide 1: Introduction -->"),
        "slide 1 marker missing\n{}",
        md
    );
    assert!(
        md.contains("<!-- slide 2: Conclusion -->"),
        "slide 2 marker missing\n{}",
        md
    );
}

#[test]
fn test_pptx_section_markers_default_off() {
    let doc = two_section_doc(FormatType::Pptx, ["Introduction", "Conclusion"]);
    let opts = RenderOptions::new();
    let md = to_markdown(&doc, &opts).unwrap();
    assert!(
        !md.contains("<!-- slide"),
        "markers must be absent by default\n{}",
        md
    );
}

#[test]
fn test_xlsx_section_markers_comment() {
    let doc = two_section_doc(FormatType::Xlsx, ["Revenue", "Costs"]);
    let opts = RenderOptions::new().with_section_markers(SectionMarkerStyle::Comment);
    let md = to_markdown(&doc, &opts).unwrap();
    assert!(
        md.contains("<!-- sheet 1: Revenue -->"),
        "sheet 1 marker missing\n{}",
        md
    );
    assert!(
        md.contains("<!-- sheet 2: Costs -->"),
        "sheet 2 marker missing\n{}",
        md
    );
}

#[test]
fn test_docx_section_markers_never_emitted() {
    let doc = two_section_doc(FormatType::Docx, ["Chapter 1", "Chapter 2"]);
    let opts = RenderOptions::new().with_section_markers(SectionMarkerStyle::Comment);
    let md = to_markdown(&doc, &opts).unwrap();
    assert!(!md.contains("<!-- "), "DOCX must not emit markers\n{}", md);
}

#[test]
fn test_pptx_nameless_section_marker() {
    let mut doc = Document::new();
    doc.format = FormatType::Pptx;
    let mut s = Section::new(0);
    s.name = None;
    doc.sections.push(s);
    let opts = RenderOptions::new().with_section_markers(SectionMarkerStyle::Comment);
    let md = to_markdown(&doc, &opts).unwrap();
    assert!(
        md.contains("<!-- slide 1 -->"),
        "nameless slide must use number-only marker\n{}",
        md
    );
}

/// Helper to create an empty resource map for tests
fn empty_resource_map() -> ResourceMap {
    HashMap::new()
}

#[test]
fn test_basic_paragraph() {
    let para = Paragraph::with_text("Hello, World!");
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    // Most punctuation is NOT escaped - only special in specific contexts
    // Only `\`, `` ` ``, `*`, `_`, `|` are always escaped
    assert_eq!(md, "Hello, World!");
}

#[test]
fn test_heading() {
    let para = Paragraph::heading(HeadingLevel::H2, "Title");
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert_eq!(md, "## Title");
}

/// A paragraph Word does not number or style whose text starts like a list item, a heading
/// or a quote stays a paragraph: the marker is escaped. A list item's own marker and a
/// heading's are written as markers; text after them is escaped the same way.
#[test]
fn text_that_starts_like_a_block_marker_stays_text() {
    let options = RenderOptions::default();
    let render = |para: &Paragraph| render_paragraph(para, &options, None, &empty_resource_map());
    for (text, md) in [
        ("* 주: 별표로 시작하는 문장", "\\* 주: 별표로 시작하는 문장"),
        ("1. 번호로 시작하는 문장", "1\\. 번호로 시작하는 문장"),
        ("# 해시로 시작하는 문장", "\\# 해시로 시작하는 문장"),
        ("> 인용처럼 보이는 문장", "\\> 인용처럼 보이는 문장"),
        ("- 대시로 시작하는 문장", "\\- 대시로 시작하는 문장"),
    ] {
        assert_eq!(render(&Paragraph::with_text(text)), md, "{text}");
    }

    let mut item = Paragraph::with_text("1. inside an item");
    item.list_info = Some(crate::model::ListInfo {
        list_type: crate::model::ListType::Bullet,
        level: 0,
        number: None,
        marker_image: None,
    });
    assert_eq!(render(&item), "- 1\\. inside an item");

    let heading = Paragraph::heading(HeadingLevel::H2, "1. Introduction");
    assert_eq!(render(&heading), "## 1. Introduction");

    let raw = RenderOptions {
        escape_special_chars: false,
        ..RenderOptions::default()
    };
    let md = render_paragraph(
        &Paragraph::with_text("# raw"),
        &raw,
        None,
        &empty_resource_map(),
    );
    assert_eq!(md, "# raw");
}

fn render_runs(runs: Vec<TextRun>) -> String {
    let mut para = Paragraph::new();
    para.runs = runs;
    render_paragraph(
        &para,
        &RenderOptions::default(),
        None,
        &empty_resource_map(),
    )
}

fn code(text: &str) -> TextRun {
    TextRun::styled(
        text,
        TextStyle {
            code: true,
            ..TextStyle::default()
        },
    )
}

#[test]
fn a_note_of_several_paragraphs_stays_one_definition() {
    let content = vec![
        Paragraph::with_text("First."),
        Paragraph::with_text("Second."),
    ];
    let md = render_note("e2", &content, &RenderOptions::default(), &HashMap::new());
    assert_eq!(md, "[^e2]: First.\n\n    Second.\n\n");
    // A label the GFM syntax cannot carry is mapped, the same way for its reference.
    assert_eq!(footnote_label("a b]"), "a-b-");
    let reference = render_runs(vec![TextRun::note_reference("a b]")]);
    assert_eq!(reference, "[^a-b-]");
    // An empty note writes nothing rather than a definition with no text.
    assert_eq!(
        render_note(
            "1",
            &[Paragraph::new()],
            &RenderOptions::default(),
            &HashMap::new()
        ),
        ""
    );
}

#[test]
fn a_code_run_is_written_raw_inside_its_fence() {
    // Nothing is escaped inside a code span: a backslash there is printed, so `C:\dir`
    // escaped would read back as `C:\\dir`, and `\`` would not keep a backtick inside.
    assert_eq!(
        render_runs(vec![TextRun::plain("Path "), code(r"C:\dir")]),
        r"Path `C:\dir`"
    );
    assert_eq!(render_runs(vec![code("a`b")]), "``a`b``");
}

#[test]
fn code_that_is_only_punctuation_is_still_code() {
    // Emphasis cannot wrap a run of punctuation between words, but a code span has no
    // flanking rule — the run used to lose its code formatting.
    assert_eq!(
        render_runs(vec![
            TextRun::plain("call"),
            code("()"),
            TextRun::plain(" now")
        ]),
        "call`()` now"
    );
}

#[test]
fn test_runs_are_written_side_by_side() {
    // The runs carry their own whitespace; a word split across two styles is one word.
    let sub = TextStyle {
        subscript: true,
        ..TextStyle::default()
    };
    let md = render_runs(vec![
        TextRun::plain("H"),
        TextRun::styled("2", sub),
        TextRun::plain("O and Hel"),
        TextRun::styled("lo", TextStyle::bold()),
        TextRun::plain(" world"),
    ]);
    assert_eq!(md, "H<sub>2</sub>O and Hel**lo** world");
}

#[test]
fn test_punctuation_against_a_word_stays_outside_the_markers() {
    let md = render_runs(vec![
        TextRun::plain("n = 32"),
        TextRun::styled(", s", TextStyle::italic()),
        TextRun::plain(" = 48"),
        TextRun::styled(",", TextStyle::italic()),
        TextRun::plain(" and"),
    ]);
    assert_eq!(md, "n = 32, *s* = 48, and");
}

#[test]
fn test_emphasis_goes_inside_a_superscript_tag() {
    let mut bold_sup = TextStyle::bold();
    bold_sup.superscript = true;
    let md = render_runs(vec![TextRun::plain("x"), TextRun::styled("2", bold_sup)]);
    assert_eq!(md, "x<sup>**2**</sup>");
}

#[test]
fn test_formatted_text() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun::styled("bold", TextStyle::bold()));
    para.runs.push(TextRun::plain(" and "));
    para.runs
        .push(TextRun::styled("italic", TextStyle::italic()));

    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert!(md.contains("**bold**"));
    assert!(md.contains("*italic*"));
}

#[test]
fn test_hyperlink() {
    let mut para = Paragraph::new();
    para.runs
        .push(TextRun::link("click here", "https://example.com"));

    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert!(md.contains("[click here](https://example.com)"));
}

#[test]
fn test_hyperlink_destination_with_a_space_is_angle_wrapped() {
    // A bare `(dest with space)` is not valid CommonMark syntax at all --
    // pulldown-cmark never produces a Link event for it, so a consumer sees the
    // brackets as literal text instead of a link.
    let mut para = Paragraph::new();
    para.runs.push(TextRun::link("doc", "my folder/file.docx"));

    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert!(
        md.contains("[doc](<my folder/file.docx>)"),
        "destination not angle-wrapped: {md:?}"
    );
}

/// A bare destination ends at an unbalanced `)`: `[doc](notes).txt)` links to `notes`.
#[test]
fn test_hyperlink_destination_with_an_unbalanced_parenthesis_is_angle_wrapped() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun::link("doc", "notes).txt"));
    let md = render_paragraph(
        &para,
        &RenderOptions::default(),
        None,
        &empty_resource_map(),
    );
    assert!(md.contains("[doc](<notes).txt>)"), "{md:?}");
}

#[test]
fn test_hyperlink_destination_with_angle_brackets_is_backslash_escaped() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun::link("doc", "a<b>c d"));

    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert!(
        md.contains("[doc](<a\\<b\\>c d>)"),
        "angle brackets not backslash-escaped: {md:?}"
    );
}

/// A `]` in the link text ends the link early unless it is escaped — and that holds with
/// `escape_special_chars` off, which is about how text looks, not where a link ends.
#[test]
fn test_hyperlink_text_with_brackets_stays_one_link() {
    let mut para = Paragraph::new();
    para.runs
        .push(TextRun::link("see [3] or ]", "https://example.com"));
    for escape_special_chars in [true, false] {
        let options = RenderOptions {
            escape_special_chars,
            ..RenderOptions::default()
        };
        let md = render_paragraph(&para, &options, None, &empty_resource_map());
        assert!(
            md.contains(r"[see \[3\] or \]](https://example.com)"),
            "escape_special_chars={escape_special_chars}: {md:?}"
        );
    }
}

#[test]
fn test_paragraph_indentation_is_not_an_indented_code_block() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::with_text("\tGBP - £"));
    section.add_paragraph(Paragraph::with_text("      six spaces"));
    section.add_paragraph(Paragraph::heading(HeadingLevel::H1, "\tMolière"));
    doc.add_section(section);

    let md = to_markdown(&doc, &RenderOptions::default()).unwrap();
    assert!(
        md.contains("\nGBP - £") || md.starts_with("GBP - £"),
        "{md:?}"
    );
    assert!(md.contains("\nsix spaces"), "{md:?}");
    assert!(md.contains("# Molière"), "{md:?}");
    assert!(!md.contains('\t'), "{md:?}");
}

#[test]
fn test_simple_table() {
    let mut table = Table::new();
    let mut header = Row::header(vec![Cell::header("A"), Cell::header("B")]);
    header.is_header = true;
    table.add_row(header);
    table.add_row(Row {
        cells: vec![Cell::with_text("1"), Cell::with_text("2")],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default();
    let md = render_table(&table, &options, &empty_resource_map());
    assert!(md.contains("| A | B |"));
    assert!(md.contains("| --- | --- |"));
    assert!(md.contains("| 1 | 2 |"));
}

#[test]
fn test_cell_runs_are_written_side_by_side_with_emphasis_off_punctuation() {
    let mut para = Paragraph::new();
    para.runs = vec![
        TextRun::plain("Hel"),
        TextRun::styled("lo", TextStyle::bold()),
        TextRun::plain(" 32"),
        TextRun::styled(", s", TextStyle::italic()),
    ];
    let mut cell = Cell::with_text("");
    cell.content = vec![para];
    let mut table = Table::new();
    table.add_row(Row::header(vec![Cell::header("A")]));
    table.add_row(Row {
        cells: vec![cell],
        is_header: false,
        height: None,
    });
    let md = render_table(&table, &RenderOptions::default(), &empty_resource_map());
    assert!(md.contains("| Hel**lo** 32, *s* |"), "{md}");
}

#[test]
fn test_document_to_markdown() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::heading(HeadingLevel::H1, "Test Document"));
    section.add_paragraph(Paragraph::with_text("This is a test."));
    doc.add_section(section);

    let options = RenderOptions::default();
    let md = to_markdown(&doc, &options).unwrap();
    assert!(md.contains("# Test Document"));
    // Period is NOT escaped (only special in ordered list context)
    assert!(md.contains("This is a test."));
}

#[test]
fn test_escape_pipe_only_in_table_cells() {
    // Outside a table cell, `|` is just a literal character.
    assert_eq!(
        escape_markdown("v1.0 | 2026-04-27", false),
        "v1.0 | 2026-04-27"
    );
    // Inside a table cell, `|` must be escaped to avoid breaking columns.
    assert_eq!(escape_markdown("a | b", true), "a \\| b");
    // A cell is a whole inline context: stars that cannot pair stay as they are.
    assert_eq!(escape_markdown("0.039***", true), "0.039***");
    assert_eq!(escape_markdown("x*y*z", true), r"x\*y\*z");
}

#[test]
fn test_escape_intra_word_underscore_not_escaped() {
    // CommonMark forbids intra-word `_` from triggering emphasis, so
    // identifiers like `snake_case` should pass through verbatim.
    assert_eq!(
        escape_markdown("YESUNG_OMS_backup_2026", false),
        "YESUNG_OMS_backup_2026"
    );
    assert_eq!(escape_markdown("in_house", false), "in_house");
    // Word-boundary `_` (next to whitespace) still needs escaping when it
    // could open an emphasis run: written bare, `a _foo_ b` is `a *foo* b`.
    assert_eq!(escape_markdown("a _foo_ b", false), r"a \_foo\_ b");
}

/// A delimiter that flanks a word may open or close emphasis, whatever is before it;
/// only one with a space on each side pairs with nothing.
#[test]
fn test_escape_star_that_can_flank_is_escaped() {
    assert_eq!(escape_markdown("say *hi* now", false), r"say \*hi\* now");
    assert_eq!(escape_markdown("(*) note", false), r"(\*) note");
    assert_eq!(escape_markdown("a * b", false), "a * b");
}

#[test]
fn test_escape_backslash_and_backtick_always() {
    assert_eq!(escape_markdown("a`b\\c", false), "a\\`b\\\\c");
}

#[test]
fn test_escape_star_intra_word_still_escaped() {
    // Unlike `_`, intra-word `*` *can* trigger emphasis, so it stays
    // escaped to be safe.
    assert_eq!(escape_markdown("foo*bar*baz", false), "foo\\*bar\\*baz");
}

#[test]
fn test_heading_strips_uniform_bold_artifact() {
    // A heading whose every run is bold (Word's typical "Heading" style)
    // should not render as `# **text**` — the bold is a styling artifact.
    let mut para = Paragraph::heading(HeadingLevel::H1, "");
    para.runs.push(TextRun::styled("Title", TextStyle::bold()));
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert_eq!(md, "# Title", "heading bold should be stripped, got {md:?}");
}

#[test]
fn test_heading_preserves_partial_bold_intent() {
    // If only part of the heading is bold, that's author intent — keep it.
    let mut para = Paragraph::heading(HeadingLevel::H2, "");
    para.runs.push(TextRun::plain("Section 2: "));
    para.runs
        .push(TextRun::styled("Required", TextStyle::bold()));
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert!(
        md.contains("**Required**"),
        "partial bold must be preserved, got {md:?}"
    );
    assert!(md.starts_with("## "));
}

#[test]
fn test_cell_alignment_falls_back_to_paragraph_alignment() {
    use crate::model::TextAlignment;
    let mut table = Table::new();
    // Header row (Left default).
    table.add_row(Row::header(vec![
        Cell::header("L"),
        Cell::header("C"),
        Cell::header("R"),
    ]));
    // Data row: paragraphs carry alignment, cells do not.
    let mut data_row = Row {
        cells: vec![
            Cell::with_text("left-text"),
            Cell::with_text("center-text"),
            Cell::with_text("right-text"),
        ],
        is_header: false,
        height: None,
    };
    data_row.cells[1].content[0].alignment = TextAlignment::Center;
    data_row.cells[2].content[0].alignment = TextAlignment::Right;
    table.add_row(data_row);

    let md = render_table(&table, &RenderOptions::default(), &empty_resource_map());
    assert!(md.contains("| --- | :---: | ---: |"), "got {md:?}");
}

#[test]
fn test_callout_blockquote_when_enabled() {
    // 1×1 fully-bold tables become `> **...**` when the option is on.
    let mut table = Table::new();
    let mut para = Paragraph::new();
    para.runs
        .push(TextRun::styled("Important note", TextStyle::bold()));
    let cell = Cell {
        content: vec![para],
        ..Cell::with_text("")
    };
    table.add_row(Row {
        cells: vec![cell],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default().with_callout_blockquote(true);
    let md = render_table(&table, &options, &empty_resource_map());
    assert!(md.starts_with("> **Important note**"), "got {md:?}");
    assert!(!md.contains("|"), "should not render as table: {md:?}");
}

#[test]
fn test_callout_blockquote_off_by_default() {
    let mut table = Table::new();
    let mut para = Paragraph::new();
    para.runs.push(TextRun::styled("X", TextStyle::bold()));
    let cell = Cell {
        content: vec![para],
        ..Cell::with_text("")
    };
    table.add_row(Row {
        cells: vec![cell],
        is_header: false,
        height: None,
    });

    let md = render_table(&table, &RenderOptions::default(), &empty_resource_map());
    assert!(md.contains("|"), "default should keep table form: {md:?}");
}

#[test]
fn test_table_cell_carriage_returns_do_not_split_rows() {
    // Defense-in-depth: even if model text carries a bare CR (e.g. a
    // Document built programmatically), a pipe-table row must stay on
    // one physical line (issue #7).
    let mut table = Table::new();
    table.add_row(Row {
        cells: vec![
            Cell::with_text("line one\rline two"),
            Cell::with_text("a\r\nb"),
        ],
        is_header: false,
        height: None,
    });

    let md = render_table(&table, &RenderOptions::default(), &empty_resource_map());
    assert!(!md.contains('\r'), "bare CR leaked into table: {md:?}");
    let row_line = md
        .lines()
        .find(|l| l.contains("line one"))
        .expect("row missing");
    assert!(
        row_line.contains("line two") && row_line.ends_with('|'),
        "row split across lines: {md:?}"
    );
}

#[test]
fn test_page_break_default_off() {
    // PageBreak no longer emits a horizontal rule unless opted in.
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::with_text("Before"));
    section.content.push(Block::PageBreak);
    section.add_paragraph(Paragraph::with_text("After"));
    doc.add_section(section);

    let md = to_markdown(&doc, &RenderOptions::default()).unwrap();
    assert!(
        !md.contains("---"),
        "page break should not emit ---: {md:?}"
    );

    let md_lossless = to_markdown(&doc, &RenderOptions::lossless()).unwrap();
    assert!(
        md_lossless.contains("---"),
        "lossless preset should emit ---"
    );
}

#[test]
fn test_headers_footers_default_off() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.header = Some(vec![Paragraph::with_text("Page header text")]);
    section.footer = Some(vec![Paragraph::with_text("Page footer text")]);
    section.add_paragraph(Paragraph::with_text("Body."));
    doc.add_section(section);

    let md = to_markdown(&doc, &RenderOptions::default()).unwrap();
    assert!(!md.contains("Page header text"), "got {md:?}");
    assert!(!md.contains("Page footer text"), "got {md:?}");
}

#[test]
fn test_consecutive_list_items_are_tight() {
    use crate::model::{ListInfo, ListType};
    let mut doc = Document::new();
    let mut section = Section::new(0);

    let mk_item = |text: &str| -> Paragraph {
        let mut p = Paragraph::with_text(text);
        p.list_info = Some(ListInfo {
            list_type: ListType::Bullet,
            level: 0,
            number: None,
            marker_image: None,
        });
        p
    };
    section.add_paragraph(mk_item("Alpha"));
    section.add_paragraph(mk_item("Bravo"));
    section.add_paragraph(mk_item("Charlie"));
    section.add_paragraph(Paragraph::with_text("After list."));
    doc.add_section(section);

    let options = RenderOptions::default();
    let md = to_markdown(&doc, &options).unwrap();

    // Tight list: items separated by single newline (no blank line between).
    assert!(
        md.contains("- Alpha\n- Bravo\n- Charlie"),
        "expected tight list, got {md:?}"
    );
    // Blank line still separates the list from the following paragraph.
    assert!(md.contains("- Charlie\n\nAfter list."), "got {md:?}");
}

/// `cleanup: None` has to mean no post-processing. Blank runs that the *document* asked
/// for are content — a consumer that diffs the output, cites it by line number, or
/// chunks it for retrieval is entitled to get them back unchanged.
#[test]
fn test_no_cleanup_leaves_document_blank_lines_alone() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    // A paragraph holding nothing but a space is an ordinary artifact of authored
    // documents, and it is the document's own vertical spacing — not the renderer's.
    section.add_paragraph(Paragraph::with_text("First."));
    section.add_paragraph(Paragraph::with_text(" "));
    section.add_paragraph(Paragraph::with_text(" "));
    section.add_paragraph(Paragraph::with_text("Second."));
    doc.add_section(section);

    let options = RenderOptions::lossless();
    assert!(options.cleanup.is_none());
    let raw = to_markdown(&doc, &options).unwrap();

    let mut collapsing = RenderOptions::lossless();
    collapsing.cleanup = Some(crate::render::options::CleanupOptions::minimal());
    let cleaned = to_markdown(&doc, &collapsing).unwrap();

    assert!(
        raw.matches('\n').count() > cleaned.matches('\n').count(),
        "cleanup must be the only thing that collapses blank lines\n  raw: {raw:?}\n  cleaned: {cleaned:?}"
    );
}

#[test]
fn test_blank_lines_collapsed_without_cleanup() {
    // A thematic break follows a paragraph that already ends with a blank line, so a
    // break that prepends its own newline yields three in a row. The renderer emits the
    // separator it actually needs instead, which is what lets this hold with no cleanup
    // configured — repairing it afterwards would mean post-processing that `cleanup:
    // None` promised not to do.
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::with_text("Before break."));
    section.content.push(Block::PageBreak);
    section.add_paragraph(Paragraph::with_text("After break."));
    doc.add_section(section);

    // Page breaks are now opt-in (F8). Use the lossless preset so the
    // PageBreak block actually emits the `---` rule whose surrounding
    // newlines we want to verify get collapsed.
    let options = RenderOptions::lossless();
    assert!(options.cleanup.is_none());

    let md = to_markdown(&doc, &options).unwrap();

    assert!(
        !md.contains("\n\n\n"),
        "output must not contain 3+ consecutive newlines: {:?}",
        md
    );
    assert!(md.contains("Before break."));
    assert!(md.contains("After break."));
    assert!(md.contains("---"));
}

#[test]
fn test_frontmatter() {
    let mut doc = Document::new();
    doc.metadata.title = Some("Test Title".to_string());
    doc.metadata.author = Some("Test Author".to_string());

    let options = RenderOptions::new().with_frontmatter(true);
    let md = to_markdown(&doc, &options).unwrap();
    assert!(md.starts_with("---\n"));
    assert!(md.contains("title: \"Test Title\""));
    assert!(md.contains("author: \"Test Author\""));
}

#[test]
fn test_korean_bullet_marker_not_heading() {
    // Paragraphs starting with Korean bullet markers should not be headings
    let para = Paragraph::heading(HeadingLevel::H2, "ㅇ항목 내용입니다");
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());

    assert!(
        !md.contains("##"),
        "Korean bullet marker should not be heading: {}",
        md
    );
    assert!(
        md.contains("ㅇ항목"),
        "Content should still be present: {}",
        md
    );
}

/// A long title is a title: the explicit style decides, and a single sentence of any
/// length does not read as body text. Measured by length, this was a plain paragraph.
#[test]
fn test_long_single_sentence_heading_kept() {
    let title =
        "3. Perspective of supply and demand balance of wood pellets and cost structure in Japan";
    assert!(title.chars().count() > 80);
    let para = Paragraph::heading(HeadingLevel::H1, title);
    let md = render_paragraph(
        &para,
        &RenderOptions::default(),
        None,
        &empty_resource_map(),
    );
    assert!(md.starts_with("# 3. Perspective"), "{}", md);
}

#[test]
fn test_long_text_not_heading() {
    // Very long text (>80 chars) should not be treated as a heading
    let long_text = "이것은 매우 긴 문장입니다. 제목으로 사용하기에는 너무 길어서 본문으로 처리되어야 합니다. 일반적인 제목은 짧고 간결해야 하며, 본문과 구분되어야 합니다.";
    assert!(
        long_text.chars().count() > 80,
        "Test text should be longer than 80 chars"
    );

    let para = Paragraph::heading(HeadingLevel::H3, long_text);
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());

    assert!(
        !md.contains("###"),
        "Long text should not have heading markers: {}",
        md
    );
    assert!(
        md.contains("이것은 매우"),
        "Content should still be present: {}",
        md
    );
}

#[test]
fn test_max_heading_level_capped() {
    // Heading levels beyond max (4) should be capped
    let para = Paragraph::heading(HeadingLevel::H6, "Deep Heading");
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());

    // Default max_heading_level is now 4
    assert!(
        md.contains("#### Deep Heading"),
        "Heading level 6 should be capped to 4: {}",
        md
    );
    assert!(
        !md.contains("######"),
        "Should not have 6 hash marks: {}",
        md
    );
}

#[test]
fn test_arrow_marker_not_heading() {
    // Paragraphs starting with arrow markers should not be headings
    let para = Paragraph::heading(HeadingLevel::H2, "→ 다음 단계로 이동");
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());

    assert!(
        !md.contains("##"),
        "Arrow marker should not be heading: {}",
        md
    );
}

#[test]
fn test_table_cell_with_bold_text() {
    let mut table = Table::new();

    // Create header row
    let header = Row::header(vec![Cell::header("Header")]);
    table.add_row(header);

    // Create data row with bold text in cell
    let mut bold_para = Paragraph::new();
    bold_para
        .runs
        .push(TextRun::styled("ClusterPlex v5.0", TextStyle::bold()));

    let cell = Cell {
        content: vec![bold_para],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: crate::model::CellAlignment::Left,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    table.add_row(Row {
        cells: vec![cell],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default();
    let md = render_table(&table, &options, &empty_resource_map());

    // Should contain bold formatting
    assert!(
        md.contains("**ClusterPlex v5.0**"),
        "Expected bold formatting, got: {}",
        md
    );
}

#[test]
fn test_table_cell_with_italic_text() {
    let mut table = Table::new();

    // Create header row
    let header = Row::header(vec![Cell::header("Header")]);
    table.add_row(header);

    // Create data row with italic text in cell
    let mut italic_para = Paragraph::new();
    italic_para
        .runs
        .push(TextRun::styled("emphasis", TextStyle::italic()));

    let cell = Cell {
        content: vec![italic_para],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: crate::model::CellAlignment::Left,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    table.add_row(Row {
        cells: vec![cell],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default();
    let md = render_table(&table, &options, &empty_resource_map());

    // Should contain italic formatting
    assert!(
        md.contains("*emphasis*"),
        "Expected italic formatting, got: {}",
        md
    );
}

#[test]
fn test_table_cell_with_multiple_paragraphs() {
    let mut table = Table::new();

    // Create header row
    let header = Row::header(vec![Cell::header("Steps")]);
    table.add_row(header);

    // Create data row with multiple paragraphs in cell
    let para1 = Paragraph::with_text("1. Active 서버 어댑터 Disable");
    let para2 = Paragraph::with_text("2. Standby 서버 어댑터 Enable");

    let cell = Cell {
        content: vec![para1, para2],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: crate::model::CellAlignment::Left,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    table.add_row(Row {
        cells: vec![cell],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default();
    let md = render_table(&table, &options, &empty_resource_map());

    // Should contain <br> between paragraphs
    assert!(
        md.contains("<br>"),
        "Expected <br> separator between paragraphs, got: {}",
        md
    );
    assert!(
        md.contains("1. Active"),
        "Expected first paragraph content, got: {}",
        md
    );
    assert!(
        md.contains("2. Standby"),
        "Expected second paragraph content, got: {}",
        md
    );
}

#[test]
fn test_table_cell_with_mixed_formatting() {
    let mut table = Table::new();

    // Create header row
    let header = Row::header(vec![Cell::header("OS"), Cell::header("리소스 타입")]);
    table.add_row(header);

    // Create data row with bold header label and normal value
    let mut para1 = Paragraph::new();
    para1.runs.push(TextRun::styled("OS", TextStyle::bold()));

    let mut para2 = Paragraph::new();
    para2.runs.push(TextRun::plain("Linux"));

    let cell1 = Cell {
        content: vec![para1],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: crate::model::CellAlignment::Left,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    let cell2 = Cell {
        content: vec![para2],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: crate::model::CellAlignment::Left,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    table.add_row(Row {
        cells: vec![cell1, cell2],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default();
    let md = render_table(&table, &options, &empty_resource_map());

    // Should contain both bold and plain text
    assert!(md.contains("**OS**"), "Expected bold OS, got: {}", md);
    assert!(md.contains("Linux"), "Expected Linux text, got: {}", md);
}

/// A run's trailing space separates it from the next run — it is not part of the link
/// label. Inside the brackets it renders in the wrong place and the raw Markdown reads
/// `](url)`, which downstream tokenizers see as one glued token.
#[test]
fn test_hyperlink_trailing_space_stays_outside_the_link() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun {
        text: "See the docs ".to_string(),
        hyperlink: Some("https://example.com".to_string()),
        ..Default::default()
    });
    para.runs.push(TextRun::plain("and continue"));

    let md = render_paragraph(
        &para,
        &RenderOptions::default(),
        None,
        &empty_resource_map(),
    );
    assert_eq!(md, "[See the docs](https://example.com) and continue");
}

/// CommonMark refuses a closing emphasis delimiter that is preceded by whitespace, so
/// `**bold **` is not bold at all — the asterisks render literally.
#[test]
fn test_emphasis_trailing_space_stays_outside_the_delimiters() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun {
        text: "bold ".to_string(),
        style: TextStyle {
            bold: true,
            ..Default::default()
        },
        ..Default::default()
    });
    para.runs.push(TextRun::plain("after"));

    let md = render_paragraph(
        &para,
        &RenderOptions::default(),
        None,
        &empty_resource_map(),
    );
    assert_eq!(md, "**bold** after");
}

/// A run that is only whitespace is the separator between its neighbours. Wrapping it in
/// markup would emit delimiters around nothing.
#[test]
fn test_whitespace_only_run_is_a_separator_not_content() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("word"));
    para.runs.push(TextRun {
        text: " ".to_string(),
        style: TextStyle {
            bold: true,
            ..Default::default()
        },
        ..Default::default()
    });
    para.runs.push(TextRun::plain("next"));

    let md = render_paragraph(
        &para,
        &RenderOptions::default(),
        None,
        &empty_resource_map(),
    );
    assert_eq!(md, "word next");
}

#[test]
fn test_line_break_rendering() {
    let mut para = Paragraph::new();
    para.runs.push(TextRun {
        text: "First line".to_string(),
        style: TextStyle::default(),
        hyperlink: None,
        line_break: true,
        page_break: false,
        revision: RevisionType::None,
        note: None,
    });
    para.runs.push(TextRun::plain("Second line"));

    // Without preserve_line_breaks option
    let options = RenderOptions::default();
    let md = render_paragraph(&para, &options, None, &empty_resource_map());
    assert!(
        !md.contains("  \n"),
        "Should not contain line break when preserve_line_breaks is false: {}",
        md
    );

    // With preserve_line_breaks option
    let options_with_breaks = RenderOptions::new().with_preserve_breaks(true);
    let md_with_breaks = render_paragraph(&para, &options_with_breaks, None, &empty_resource_map());
    assert!(
        md_with_breaks.contains("First line  \n"),
        "Should contain Markdown line break: {}",
        md_with_breaks
    );
    assert!(
        md_with_breaks.contains("Second line"),
        "Should contain second line: {}",
        md_with_breaks
    );
}

#[test]
fn test_table_cell_line_break_rendering_with_preserve_breaks() {
    let mut table = Table::new();
    table.add_row(Row::header(vec![Cell::header("Notes")]));
    table.add_row(Row {
        cells: vec![Cell {
            content: vec![Paragraph {
                runs: vec![
                    TextRun {
                        text: "First line".to_string(),
                        line_break: true,
                        ..Default::default()
                    },
                    TextRun::plain("Second line"),
                ],
                ..Default::default()
            }],
            nested_tables: Vec::new(),
            col_span: 1,
            row_span: 1,
            alignment: CellAlignment::Left,
            vertical_alignment: crate::model::VerticalAlignment::Top,
            is_header: false,
            background: None,
            background_image: None,
        }],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::new().with_preserve_breaks(true);
    let md = render_table(&table, &options, &empty_resource_map());

    assert!(
        md.contains("First line<br>Second line"),
        "Expected preserved line break in table cell, got: {md}"
    );
}

/// A table of one row, `merged` spanning two columns, holding `nested` in its first cell.
fn holder_with_nested(nested: Table) -> Table {
    let mut first = Cell::with_text("holder");
    first.nested_tables.push(nested);
    let mut spanning = Cell::with_text("wide");
    spanning.col_span = 2;
    let mut table = Table::new();
    table.add_row(row_of(vec![first, spanning]));
    table
}

fn row_of(cells: Vec<Cell>) -> Row {
    Row {
        cells,
        ..Row::default()
    }
}

fn inner_table() -> Table {
    let mut inner = Table::new();
    inner.add_row(row_of(vec![
        Cell::with_text("inner a"),
        Cell::with_text("inner b"),
    ]));
    inner
}

#[test]
fn a_nested_table_follows_a_holder_written_as_html() {
    // A merged cell sends the holder to the HTML fallback, which used to return before
    // the nested tables were written: they vanished from the output.
    let options = RenderOptions::new().with_table_fallback(crate::render::TableFallback::Html);
    let md = render_table(
        &holder_with_nested(inner_table()),
        &options,
        &empty_resource_map(),
    );
    assert!(md.contains("<table>"), "holder is HTML, got: {md}");
    let holder_at = md.find("holder").expect("holder text");
    let nested_at = md
        .find("inner a")
        .expect("the nested table must be written");
    assert!(
        holder_at < nested_at,
        "nested follows its holder, got: {md}"
    );
}

#[test]
fn a_nested_table_follows_a_holder_written_as_a_callout() {
    // A 1x1 emphasized table is written as a blockquote; the nested table is not
    // part of the quote and must still be written.
    let mut cell = Cell::with_text("note");
    for run in cell.content[0].runs.iter_mut() {
        run.style.bold = true;
    }
    cell.nested_tables.push(inner_table());
    let mut table = Table::new();
    table.add_row(row_of(vec![cell]));
    let mut options = RenderOptions::new();
    options.callout_blockquote = true;
    let md = render_table(&table, &options, &empty_resource_map());
    assert!(md.contains("> **note**"), "holder is a callout, got: {md}");
    assert!(
        md.contains("inner a"),
        "nested table must be written, got: {md}"
    );
}

#[test]
fn test_html_table_fallback_escapes_special_chars() {
    let mut table = Table::new();
    table.add_row(Row::header(vec![Cell::header("Header")]));
    table.add_row(Row {
        cells: vec![Cell {
            content: vec![Paragraph::with_text("<unsafe> & value")],
            nested_tables: Vec::new(),
            col_span: 2,
            row_span: 1,
            alignment: CellAlignment::Left,
            vertical_alignment: crate::model::VerticalAlignment::Top,
            is_header: false,
            background: None,
            background_image: None,
        }],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::new().with_table_fallback(crate::render::TableFallback::Html);
    let md = render_table(&table, &options, &empty_resource_map());

    assert!(
        md.contains("&lt;unsafe&gt; &amp; value"),
        "Expected HTML-escaped table cell content, got: {md}"
    );
}

#[test]
fn test_table_cell_alignment_rendering() {
    let mut table = Table::new();

    // Create header row
    let header = Row::header(vec![
        Cell::header("Left"),
        Cell::header("Center"),
        Cell::header("Right"),
    ]);
    table.add_row(header);

    // Create data row with different alignments
    let left_cell = Cell {
        content: vec![Paragraph::with_text("L")],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: CellAlignment::Left,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    let center_cell = Cell {
        content: vec![Paragraph::with_text("C")],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: CellAlignment::Center,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    let right_cell = Cell {
        content: vec![Paragraph::with_text("R")],
        nested_tables: Vec::new(),
        col_span: 1,
        row_span: 1,
        alignment: CellAlignment::Right,
        vertical_alignment: crate::model::VerticalAlignment::Top,
        is_header: false,
        background: None,
        background_image: None,
    };

    table.add_row(Row {
        cells: vec![left_cell, center_cell, right_cell],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::default();
    let md = render_table(&table, &options, &empty_resource_map());

    // Should contain alignment markers in separator row
    assert!(
        md.contains("| --- |"),
        "Expected left alignment marker, got: {}",
        md
    );
    assert!(
        md.contains("| :---: |"),
        "Expected center alignment marker, got: {}",
        md
    );
    assert!(
        md.contains("| ---: |"),
        "Expected right alignment marker, got: {}",
        md
    );
}

#[test]
fn test_render_header_footer() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.header = Some(vec![Paragraph::with_text("My Header")]);
    section.footer = Some(vec![Paragraph::with_text("Page 1 of 10")]);
    section.add_paragraph(Paragraph::with_text("Body content"));
    doc.add_section(section);

    // Header/Footer rendering is now opt-in (F10).
    let options = RenderOptions::lossless();
    let md = to_markdown(&doc, &options).unwrap();

    assert!(
        md.contains("> *Header: My Header*"),
        "Expected header in output, got: {}",
        md
    );
    assert!(
        md.contains("> *Footer: Page 1 of 10*"),
        "Expected footer in output, got: {}",
        md
    );
    // Verify ordering: header before body, footer after body
    let header_pos = md.find("> *Header:").unwrap();
    let body_pos = md.find("Body content").unwrap();
    let footer_pos = md.find("> *Footer:").unwrap();
    assert!(header_pos < body_pos, "Header should appear before body");
    assert!(body_pos < footer_pos, "Footer should appear after body");
}

#[test]
fn test_render_header_footer_multiple_paragraphs() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.header = Some(vec![
        Paragraph::with_text("Company"),
        Paragraph::with_text("Department"),
    ]);
    section.add_paragraph(Paragraph::with_text("Content"));
    doc.add_section(section);

    let options = RenderOptions::lossless();
    let md = to_markdown(&doc, &options).unwrap();

    assert!(
        md.contains("> *Header: Company | Department*"),
        "Multiple header paragraphs should be joined with ' | ', got: {}",
        md
    );
}

#[test]
fn test_render_no_header_footer() {
    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::with_text("Body only"));
    doc.add_section(section);

    let options = RenderOptions::default();
    let md = to_markdown(&doc, &options).unwrap();

    assert!(!md.contains("Header:"), "No header should be rendered");
    assert!(!md.contains("Footer:"), "No footer should be rendered");
}

#[test]
fn test_html_table_vmerge_rowspan() {
    // BUG-2: vMerge origin cells must carry correct rowspan in HTML fallback
    let mut table = Table::new();

    // Row 0: [A(rowspan=2), B]
    table.add_row(Row {
        cells: vec![
            Cell {
                content: vec![Paragraph::with_text("A")],
                nested_tables: vec![],
                col_span: 1,
                row_span: 2,
                alignment: CellAlignment::Left,
                vertical_alignment: crate::model::VerticalAlignment::Top,
                is_header: false,
                background: None,
                background_image: None,
            },
            Cell {
                content: vec![Paragraph::with_text("B")],
                nested_tables: vec![],
                col_span: 1,
                row_span: 1,
                alignment: CellAlignment::Left,
                vertical_alignment: crate::model::VerticalAlignment::Top,
                is_header: false,
                background: None,
                background_image: None,
            },
        ],
        is_header: false,
        height: None,
    });
    // Row 1: [C] (A's continuation is absent from cells, C is at col 1)
    table.add_row(Row {
        cells: vec![Cell {
            content: vec![Paragraph::with_text("C")],
            nested_tables: vec![],
            col_span: 1,
            row_span: 1,
            alignment: CellAlignment::Left,
            vertical_alignment: crate::model::VerticalAlignment::Top,
            is_header: false,
            background: None,
            background_image: None,
        }],
        is_header: false,
        height: None,
    });

    let options = RenderOptions::new().with_table_fallback(crate::render::TableFallback::Html);
    let html = render_table(&table, &options, &empty_resource_map());

    assert!(
        html.contains("rowspan=\"2\""),
        "Expected rowspan=2 on origin cell, got: {html}"
    );
}

#[test]
fn test_list_item_with_heading_style_renders_as_list_not_heading() {
    // BUG-1: paragraph with both list_info and heading style must not produce "# -"
    use crate::model::{ListInfo, ListType};

    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("Item text"));
    para.heading = HeadingLevel::from_number(1);
    para.list_info = Some(ListInfo {
        level: 0,
        number: None,
        marker_image: None,
        list_type: ListType::Bullet,
    });

    let mut doc = Document::new();
    let mut section = Section::new(0);
    section.add_paragraph(para);
    doc.add_section(section);

    let options = RenderOptions::default();
    let md = to_markdown(&doc, &options).unwrap();

    assert!(
        !md.contains("# "),
        "Heading marker must not appear for list item, got: {md}"
    );
    assert!(
        md.contains("- Item text"),
        "List marker must appear, got: {md}"
    );
}

#[test]
fn test_frontmatter_format_field() {
    // FEAT-3: frontmatter must include format field
    use crate::detect::FormatType;

    let mut doc = Document::new();
    doc.format = FormatType::Docx;
    let mut section = Section::new(0);
    section.add_paragraph(Paragraph::with_text("Content"));
    doc.add_section(section);

    let options = RenderOptions::new().with_frontmatter(true);
    let md = to_markdown(&doc, &options).unwrap();

    assert!(
        md.contains("format: docx"),
        "Expected 'format: docx' in frontmatter, got: {md}"
    );
}
