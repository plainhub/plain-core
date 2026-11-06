//! Markdown renderer implementation.

use std::collections::HashMap;
use std::ops::Range;

use crate::detect::FormatType;
use crate::error::Result;
use crate::model::{
    Block, Cell, CellAlignment, Document, HeadingLevel, Paragraph, RevisionType, Table, TextRun,
};

use super::heading_analyzer::{HeadingAnalyzer, HeadingDecision};
use super::options::{RenderOptions, RevisionHandling, SectionMarkerStyle};
use crate::markdown::{self, emphasis_span};

/// Map of resource IDs to their filenames
type ResourceMap = HashMap<String, String>;

/// Convert a Document to Markdown.
pub fn to_markdown(doc: &Document, options: &RenderOptions) -> Result<String> {
    crate::limits::check_document(doc)?;
    // If heading analysis is enabled, use the analyzer
    if let Some(ref config) = options.heading_config {
        return to_markdown_with_analyzer(doc, options, config);
    }

    // Standard rendering without sophisticated heading analysis
    to_markdown_standard(doc, options)
}

/// Render a single section to Markdown.
///
/// This is the streaming counterpart to [`to_markdown`]: it renders one
/// section at a time without requiring a full `Document`. Heading analysis
/// is not available on this path (requires all sections); heading levels come
/// from `section.content` directly.
///
/// `resource_map` maps resource IDs to filenames (e.g., `"rId1" → "image1.png"`).
/// In the streaming path, supply the `image_map` from [`crate::ParseEvent::DocumentStart`].
pub fn render_section_to_string(
    section: &crate::model::Section,
    section_index: usize,
    doc_format: crate::detect::FormatType,
    options: &RenderOptions,
    resource_map: &HashMap<String, String>,
) -> String {
    let mut output = String::new();
    render_section_impl(
        section,
        section_index,
        doc_format,
        options,
        resource_map,
        &mut output,
    );
    output
}

/// Build a map from resource IDs to their suggested filenames.
fn build_resource_map(doc: &Document) -> ResourceMap {
    doc.resources
        .iter()
        .map(|(id, resource)| (id.clone(), resource.suggested_filename(id)))
        .collect()
}

/// Render header or footer paragraphs as a blockquote with italic label.
fn render_header_footer(
    label: &str,
    paragraphs: &[Paragraph],
    options: &RenderOptions,
    resource_map: &ResourceMap,
    output: &mut String,
) {
    let texts: Vec<String> = paragraphs
        .iter()
        .map(|p| render_paragraph(p, options, None, resource_map))
        .filter(|t| !t.is_empty())
        .collect();
    if !texts.is_empty() {
        output.push_str(&format!("> *{}: {}*\n\n", label, texts.join(" | ")));
    }
}

/// Get image path from resource ID, resolving to actual filename if available. The path is
/// raw; [`markdown::image`] writes it as a destination.
fn resolve_image_path(resource_id: &str, resource_map: &ResourceMap, prefix: &str) -> String {
    let filename = resource_map
        .get(resource_id)
        .cloned()
        .unwrap_or_else(|| resource_id.to_string());
    format!("{}{}", prefix, filename)
}

/// Build a section boundary marker comment for PPTX slides or XLSX sheets.
///
/// Returns an empty string when markers are disabled or the format is DOCX.
fn section_marker(
    format: FormatType,
    style: SectionMarkerStyle,
    idx: usize,
    name: Option<&str>,
) -> String {
    if style == SectionMarkerStyle::None {
        return String::new();
    }
    let n = idx + 1;
    match format {
        FormatType::Pptx | FormatType::Ppt => match name.filter(|s| !s.is_empty()) {
            Some(name) => format!("<!-- slide {}: {} -->", n, name),
            None => format!("<!-- slide {} -->", n),
        },
        FormatType::Xlsx | FormatType::Xls => match name.filter(|s| !s.is_empty()) {
            Some(name) => format!("<!-- sheet {}: {} -->", n, name),
            None => format!("<!-- sheet {} -->", n),
        },
        FormatType::Docx | FormatType::Doc => String::new(),
    }
}

/// Core per-section render, shared by the batch and streaming paths.
fn render_section_impl(
    section: &crate::model::Section,
    section_index: usize,
    doc_format: FormatType,
    options: &RenderOptions,
    resource_map: &ResourceMap,
    output: &mut String,
) {
    let marker = section_marker(
        doc_format,
        options.section_markers,
        section_index,
        section.name.as_deref(),
    );
    if !marker.is_empty() {
        output.push_str(&marker);
        output.push_str("\n\n");
    }

    if let Some(ref name) = section.name {
        if section_index > 0 {
            output.push_str("\n---\n\n");
        }
        output.push_str(&format!("## {}\n\n", name));
    }

    if options.include_headers_footers {
        if let Some(ref header) = section.header {
            render_header_footer("Header", header, options, resource_map, output);
        }
    }

    for (block_idx, block) in section.content.iter().enumerate() {
        match block {
            Block::Paragraph(para) => {
                let md = render_paragraph(para, options, None, resource_map);
                if !md.is_empty() || options.include_empty_paragraphs {
                    output.push_str(&md);
                    let in_list = para.list_info.is_some();
                    let tight = in_list && next_block_continues_list(&section.content, block_idx);
                    if tight {
                        output.push('\n');
                    } else if options.paragraph_spacing {
                        output.push_str("\n\n");
                    } else {
                        output.push('\n');
                    }
                }
            }
            Block::Table(table) => {
                output.push_str(&render_table(table, options, resource_map));
                output.push_str("\n\n");
            }
            Block::Note { label, content } => {
                output.push_str(&render_note(label, content, options, resource_map));
            }
            Block::PageBreak => {
                if options.emit_page_breaks {
                    push_thematic_break(output);
                }
            }
            Block::SectionBreak => {
                push_thematic_break(output);
            }
            Block::Image {
                resource_id,
                alt_text,
                ..
            } => {
                if options.include_images {
                    let alt = alt_text.as_deref().unwrap_or("image");
                    let path =
                        resolve_image_path(resource_id, resource_map, &options.image_path_prefix);
                    output.push_str(&markdown::image(alt, &path, false));
                    output.push_str("\n\n");
                }
            }
        }
    }

    if options.include_headers_footers {
        if let Some(ref footer) = section.footer {
            render_header_footer("Footer", footer, options, resource_map, output);
        }
    }

    if let Some(ref notes) = section.notes {
        if !notes.is_empty() {
            output.push_str("\n> **Notes:**\n");
            for note in notes {
                let text = render_paragraph(note, options, None, resource_map);
                if !text.is_empty() {
                    output.push_str(&format!("> {}\n", text));
                }
            }
            output.push('\n');
        }
    }
}

/// Standard markdown conversion (without heading analyzer).
fn to_markdown_standard(doc: &Document, options: &RenderOptions) -> Result<String> {
    let mut output = String::new();
    let resource_map = build_resource_map(doc);

    if options.include_frontmatter {
        output.push_str(&render_frontmatter(doc));
    }

    for (i, section) in doc.sections.iter().enumerate() {
        render_section_impl(section, i, doc.format, options, &resource_map, &mut output);
    }

    Ok(finalize(output, options))
}

/// Finish a rendered document: apply the configured cleanup, if any.
///
/// `RenderOptions::cleanup == None` means no post-processing, and nothing here may quietly
/// contradict that. Blank-line collapsing used to run unconditionally on the grounds that it
/// is lossless under CommonMark. That holds for the *rendered* result, but not for the
/// Markdown itself: a consumer diffing the output, citing it by line number, or chunking it
/// for retrieval sees different text. Collapsing is part of cleanup's whitespace
/// normalization now, which is also what makes Markdown and text output agree.
///
/// Trimming the document's own ends stays unconditional — those blank lines are the
/// renderer's scaffolding between sections, not content it was given.
fn finalize(output: String, options: &RenderOptions) -> String {
    let processed = match options.cleanup {
        Some(ref cleanup) => super::cleanup::clean_text(&output, cleanup),
        None => output,
    };

    let trimmed = processed.trim().to_string();

    trimmed
}

/// Convert a Document to Markdown with sophisticated heading analysis.
fn to_markdown_with_analyzer(
    doc: &Document,
    options: &RenderOptions,
    config: &super::heading_analyzer::HeadingConfig,
) -> Result<String> {
    // Run two-pass heading analysis
    let mut analyzer = HeadingAnalyzer::new(config.clone());
    let decisions = analyzer.analyze(doc);

    let mut output = String::new();
    let resource_map = build_resource_map(doc);

    // Add frontmatter if requested
    if options.include_frontmatter {
        output.push_str(&render_frontmatter(doc));
    }

    // Render each section with pre-computed heading decisions
    for (section_idx, section) in doc.sections.iter().enumerate() {
        // Section boundary marker (PPTX/XLSX only, opt-in)
        let marker = section_marker(
            doc.format,
            options.section_markers,
            section_idx,
            section.name.as_deref(),
        );
        if !marker.is_empty() {
            output.push_str(&marker);
            output.push_str("\n\n");
        }

        // Add section name as heading if present
        if let Some(ref name) = section.name {
            if section_idx > 0 {
                output.push_str("\n---\n\n");
            }
            output.push_str(&format!("## {}\n\n", name));
        }

        // Render header if present (DOCX)
        if options.include_headers_footers {
            if let Some(ref header) = section.header {
                render_header_footer("Header", header, options, &resource_map, &mut output);
            }
        }

        // Get decisions for this section
        let section_decisions = decisions.get(section_idx);

        // Track paragraph index within section (only count Paragraph blocks)
        let mut para_idx = 0;

        // Render content blocks
        for (block_idx, block) in section.content.iter().enumerate() {
            match block {
                Block::Paragraph(para) => {
                    // Get the pre-computed decision for this paragraph
                    let decision = section_decisions.and_then(|d| d.get(para_idx)).copied();
                    let md = render_paragraph(para, options, decision, &resource_map);

                    if !md.is_empty() || options.include_empty_paragraphs {
                        output.push_str(&md);
                        let in_list = para.list_info.is_some();
                        let tight =
                            in_list && next_block_continues_list(&section.content, block_idx);
                        if tight {
                            output.push('\n');
                        } else if options.paragraph_spacing {
                            output.push_str("\n\n");
                        } else {
                            output.push('\n');
                        }
                    }

                    para_idx += 1;
                }
                Block::Table(table) => {
                    output.push_str(&render_table(table, options, &resource_map));
                    output.push_str("\n\n");
                }
                Block::Note { label, content } => {
                    output.push_str(&render_note(label, content, options, &resource_map));
                }
                Block::PageBreak => {
                    if options.emit_page_breaks {
                        push_thematic_break(&mut output);
                    }
                }
                Block::SectionBreak => {
                    push_thematic_break(&mut output);
                }
                Block::Image {
                    resource_id,
                    alt_text,
                    ..
                } => {
                    if options.include_images {
                        let alt = alt_text.as_deref().unwrap_or("image");
                        let path = resolve_image_path(
                            resource_id,
                            &resource_map,
                            &options.image_path_prefix,
                        );
                        output.push_str(&markdown::image(alt, &path, false));
                        output.push_str("\n\n");
                    }
                }
            }
        }

        // Render footer if present (DOCX)
        if options.include_headers_footers {
            if let Some(ref footer) = section.footer {
                render_header_footer("Footer", footer, options, &resource_map, &mut output);
            }
        }

        // Render notes if present (for PPTX)
        if let Some(ref notes) = section.notes {
            if !notes.is_empty() {
                output.push_str("\n> **Notes:**\n");
                for note in notes {
                    // Notes don't use heading analysis
                    let text = render_paragraph(note, options, None, &resource_map);
                    if !text.is_empty() {
                        output.push_str(&format!("> {}\n", text));
                    }
                }
                output.push('\n');
            }
        }
    }

    Ok(finalize(output, options))
}

/// Render YAML frontmatter from document metadata.
fn render_frontmatter(doc: &Document) -> String {
    let mut fm = String::from("---\n");
    let meta = &doc.metadata;

    // Document format (docx / xlsx / pptx)
    fm.push_str(&format!("format: {}\n", doc.format.extension()));

    // Core metadata
    if let Some(ref title) = meta.title {
        fm.push_str(&format!("title: \"{}\"\n", escape_yaml(title)));
    }
    if let Some(ref author) = meta.author {
        fm.push_str(&format!("author: \"{}\"\n", escape_yaml(author)));
    }
    if let Some(ref subject) = meta.subject {
        fm.push_str(&format!("subject: \"{}\"\n", escape_yaml(subject)));
    }

    // Dates
    if let Some(ref created) = meta.created {
        fm.push_str(&format!("created: \"{}\"\n", created));
    }
    if let Some(ref modified) = meta.modified {
        fm.push_str(&format!("modified: \"{}\"\n", modified));
    }

    // Document statistics
    if let Some(page_count) = meta.page_count {
        // Use appropriate label based on document type (inferred from section names)
        let label = if doc
            .sections
            .first()
            .and_then(|s| s.name.as_ref())
            .is_some_and(|n| n.starts_with("Slide"))
        {
            "slides"
        } else if doc
            .sections
            .first()
            .and_then(|s| s.name.as_ref())
            .is_some_and(|n| n.starts_with("Sheet"))
        {
            "sheets"
        } else {
            "pages"
        };
        fm.push_str(&format!("{}: {}\n", label, page_count));
    }
    if let Some(word_count) = meta.word_count {
        fm.push_str(&format!("words: {}\n", word_count));
    }

    // Keywords as YAML list
    if !meta.keywords.is_empty() {
        fm.push_str("keywords:\n");
        for keyword in &meta.keywords {
            fm.push_str(&format!("  - \"{}\"\n", escape_yaml(keyword)));
        }
    }

    // Application info
    if let Some(ref app) = meta.application {
        fm.push_str(&format!("application: \"{}\"\n", escape_yaml(app)));
    }

    fm.push_str("---\n\n");
    fm
}

/// Escape special characters in YAML strings.
fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Render a paragraph to Markdown.
///
/// If `heading_decision` is provided (from HeadingAnalyzer), it takes precedence
/// over the simple heading detection logic.
fn render_paragraph(
    para: &Paragraph,
    options: &RenderOptions,
    heading_decision: Option<HeadingDecision>,
    resource_map: &ResourceMap,
) -> String {
    let mut output = String::new();

    // Merge adjacent runs with the same style to avoid issues like:
    // **시** **험** **합** -> **시험합**
    let merged_para = para.with_merged_runs();

    // Handle heading based on decision or fallback to simple logic
    let effective_heading: Option<HeadingLevel> = if let Some(decision) = heading_decision {
        // Use analyzer's decision
        match decision {
            HeadingDecision::Explicit(level) | HeadingDecision::Inferred(level) => Some(level),
            HeadingDecision::Demoted | HeadingDecision::None => None,
        }
    } else {
        // No analyzer: the explicit heading style decides, under the same exclusion the
        // analyzer applies to it.
        if merged_para.heading.is_heading() {
            let plain_text = merged_para.plain_text();
            if !super::heading_analyzer::explicit_heading_misused(plain_text.trim()) {
                let level = merged_para.heading.level().min(options.max_heading_level);
                Some(HeadingLevel::from_number(level))
            } else {
                None
            }
        } else {
            None
        }
    };

    // List items are never headings — suppress heading if list_info is set
    let effective_heading = if merged_para.list_info.is_some() {
        None
    } else {
        effective_heading
    };

    // Apply heading formatting if determined
    if let Some(level) = effective_heading {
        let capped_level = level.level().min(options.max_heading_level);
        if capped_level > 0 {
            output.push_str(&"#".repeat(capped_level as usize));
            output.push(' ');
        }
    }

    // Handle list items
    if let Some(ref list_info) = merged_para.list_info {
        let indent = "  ".repeat(list_info.level as usize);
        output.push_str(&indent);
        match list_info.list_type {
            crate::model::ListType::Bullet => {
                output.push(options.list_marker);
                output.push(' ');
            }
            crate::model::ListType::Numbered => {
                let num = list_info.number.unwrap_or(1);
                output.push_str(&format!("{}. ", num));
            }
            crate::model::ListType::None => {}
        }
    }

    // Decide whether the heading text is uniformly emphasized (a styling
    // artifact). Partial emphasis stays untouched.
    let suppress_heading_emphasis = effective_heading.is_some()
        && options.strip_redundant_emphasis_in_headings
        && all_runs_uniformly_bold(&merged_para);

    // The runs carry their own whitespace (`xml:space="preserve"`), so they are written
    // side by side: a word split across two styles is still one word.
    let run_ctx = RunContext {
        in_table_cell: false,
        suppress_emphasis: suppress_heading_emphasis,
    };
    let body_start = output.len();
    // A heading is one line: a hard break, or the thematic break of a page, written inside it
    // would end the heading after its first line. Its breaks are spaces, as without the options.
    let heading_options;
    let run_options = if effective_heading.is_some()
        && (options.preserve_line_breaks || options.emit_page_breaks)
    {
        heading_options = RenderOptions {
            preserve_line_breaks: false,
            emit_page_breaks: false,
            ..options.clone()
        };
        &heading_options
    } else {
        options
    };
    let mut page_breaks = render_runs(&merged_para.runs, run_options, run_ctx, &mut output);
    // Markdown has no paragraph indentation: a first line opening with a tab or four
    // spaces is an indented code block, so a paragraph indented with a tab stop would
    // come out as code. The indentation carries no text, so it is dropped — from a line
    // with text after it only: a paragraph of nothing but spaces is the document's own
    // spacing, kept as it is, and cannot open a code block.
    let rest = output[body_start..].trim_start_matches([' ', '\t']);
    if !rest.is_empty() {
        let indent = output.len() - body_start - rest.len();
        output.replace_range(body_start..body_start + indent, "");
        for marker in &mut page_breaks {
            *marker = marker.start - indent..marker.end - indent;
        }
    }
    // The paragraph's own markers -- a heading's `#`, a list item's -- are written above; text
    // that merely starts like one (`1. `, `* `, `# ` in a paragraph Word does not number or
    // style) is the document's text and stays text. A heading's content opens no block.
    if options.escape_special_chars && effective_heading.is_none() {
        // Each piece between the thematic breaks of page breaks opens a block of its own; the
        // breaks themselves are written as they are.
        let mut body = String::new();
        let mut at = body_start;
        for marker in &page_breaks {
            body.push_str(&markdown::escape_block_starts(&output[at..marker.start]));
            body.push_str(&output[marker.clone()]);
            at = marker.end;
        }
        body.push_str(&markdown::escape_block_starts(&output[at..]));
        output.replace_range(body_start.., &body);
    }

    // Render inline images
    for image in para.images.iter().filter(|_| options.include_images) {
        if !output.is_empty() {
            output.push('\n');
        }
        let alt = image.alt_text.as_deref().unwrap_or("image");
        let path = resolve_image_path(&image.resource_id, resource_map, &options.image_path_prefix);
        output.push_str(&markdown::image(alt, &path, false));
    }

    output
}

/// True when the next block is another list paragraph — used to decide
/// whether the separator after the current list item should be tight
/// (`\n`) or loose (`\n\n`).
fn next_block_continues_list(blocks: &[Block], idx: usize) -> bool {
    matches!(
        blocks.get(idx + 1),
        Some(Block::Paragraph(p)) if p.list_info.is_some()
    )
}

/// True when every non-empty run in the paragraph carries `bold`. This
/// signals that the bold is a styling artifact (e.g. a Heading style with a
/// blanket `<w:b/>` run property), not author-applied emphasis on a slice.
fn all_runs_uniformly_bold(para: &Paragraph) -> bool {
    let mut saw_text = false;
    for run in &para.runs {
        if run.text.trim().is_empty() {
            continue;
        }
        saw_text = true;
        if !run.style.bold {
            return false;
        }
    }
    saw_text
}

/// Context flags passed to [`render_run`].
#[derive(Debug, Clone, Copy, Default)]
struct RunContext {
    /// True when rendering inside a markdown table cell (`|` must be escaped).
    in_table_cell: bool,
    /// True when surrounding container (heading, header cell) is uniformly
    /// emphasized — bold/italic on individual runs is treated as a styling
    /// artifact and stripped.
    suppress_emphasis: bool,
}

/// Append a thematic break, separated from what precedes it by exactly one blank line.
///
/// A block-level element cannot see what came before it, so prepending a newline
/// unconditionally is a guess — and when the previous paragraph already ended with a blank
/// line, that guess produced a run of three newlines which a later pass had to collapse.
/// Normalizing the separator here means nothing downstream has to repair it.
fn push_thematic_break(output: &mut String) {
    while output.ends_with('\n') {
        output.pop();
    }
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str("---\n\n");
}

/// Whether the run ends a line or a page, whatever the options make of it.
fn ends_a_line(run: &TextRun) -> bool {
    run.line_break || run.page_break
}

/// Write `runs` one after another onto `out`.
///
/// The runs carry their own whitespace (`xml:space="preserve"`), so they are written side by
/// side: a word split across two styles is still one word. A break is the exception. It ends a
/// line, so the words on either side of it are two words, and when the options ask for no marker
/// ([`break_marker`] is empty) a Markdown paragraph has a space there -- the one a reflowed line
/// ends with. Writing nothing fused them (`Zero<w:br/>Charlie` read `ZeroCharlie`). The space is
/// written once the next text arrives, so a break that ends the paragraph, or sits against a
/// space already there, adds none -- and a break in a run that rendered nothing still counts.
/// Returns where the thematic breaks of emitted page breaks stand in `out`, which the caller
/// must not escape as text.
fn render_runs(
    runs: &[TextRun],
    options: &RenderOptions,
    ctx: RunContext,
    out: &mut String,
) -> Vec<Range<usize>> {
    let start = out.len();
    let mut apart = false;
    let mut page_breaks = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let after = runs.get(i + 1).and_then(|next| next.text.chars().next());
        let text = render_run(run, options, ctx, out.chars().next_back(), after);
        if apart && !text.is_empty() {
            let touching = out.len() > start
                && !out.ends_with(char::is_whitespace)
                && !text.starts_with(char::is_whitespace);
            if touching {
                out.push(' ');
            }
            apart = false;
        }
        out.push_str(&text);
        if run.page_break && options.emit_page_breaks {
            // The marker ends the run's text; it is structure, not text to be escaped.
            page_breaks.push(out.len() - PAGE_BREAK_MARKER.len()..out.len());
        }
        if ends_a_line(run) && break_marker(run, options).is_empty() {
            apart = true;
        }
    }
    page_breaks
}

/// A page break written inside a paragraph: a thematic break between two blocks.
const PAGE_BREAK_MARKER: &str = "\n\n---\n\n";

/// The break marker a run ends with, if the options ask for it.
///
/// A page break wins over a line break, and both are suffixes: they follow whatever the run
/// rendered, including a run that rendered nothing at all.
fn break_marker(run: &TextRun, options: &RenderOptions) -> &'static str {
    if run.page_break && options.emit_page_breaks {
        PAGE_BREAK_MARKER
    } else if run.line_break && options.preserve_line_breaks {
        "  \n"
    } else {
        ""
    }
}

/// A note label as a GFM footnote label: whitespace and the characters that would end or
/// break the label (`[`, `]`, `\`, `^`) become `-`. Labels the parsers give (`1`, `e2`) pass
/// through; a reference and its definition map the same label to the same text.
fn footnote_label(label: &str) -> String {
    label
        .chars()
        .map(|c| match c {
            '[' | ']' | '\\' | '^' => '-',
            c if c.is_whitespace() => '-',
            c => c,
        })
        .collect()
}

/// A footnote definition, `[^label]: text`: the first paragraph follows the label, each later
/// one is indented four spaces after a blank line, which keeps it inside the definition.
/// A note with no text writes nothing.
fn render_note(
    label: &str,
    content: &[Paragraph],
    options: &RenderOptions,
    resource_map: &ResourceMap,
) -> String {
    let paragraphs: Vec<String> = content
        .iter()
        .map(|para| render_paragraph(para, options, None, resource_map))
        .filter(|md| !md.trim().is_empty())
        .collect();
    if paragraphs.is_empty() {
        return String::new();
    }
    let mut out = format!("[^{}]: ", footnote_label(label));
    for (i, md) in paragraphs.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        for (j, line) in md.lines().enumerate() {
            if j > 0 {
                out.push('\n');
            }
            if (i > 0 || j > 0) && !line.is_empty() {
                out.push_str("    ");
            }
            out.push_str(line);
        }
    }
    out.push_str("\n\n");
    out
}

/// Render one run. `before` is the last character already written and `after` the first
/// one the next run will write — what the run's emphasis delimiters land between.
fn render_run(
    run: &TextRun,
    options: &RenderOptions,
    ctx: RunContext,
    before: Option<char>,
    after: Option<char>,
) -> String {
    // Handle tracked changes based on revision_handling option
    match (&run.revision, &options.revision_handling) {
        // AcceptAll: show inserted text, hide deleted text
        (RevisionType::Deleted, RevisionHandling::AcceptAll) => {
            // Hidden text still yields its break marker, if any.
            return break_marker(run, options).to_string();
        }
        // RejectAll: show deleted text, hide inserted text
        (RevisionType::Inserted, RevisionHandling::RejectAll) => {
            return break_marker(run, options).to_string();
        }
        // ShowMarkup or normal text: continue with rendering
        _ => {}
    }

    if let Some(label) = &run.note {
        return format!("[^{}]{}", footnote_label(label), break_marker(run, options));
    }

    let effective_bold = run.style.bold && !ctx.suppress_emphasis;
    let effective_italic = run.style.italic && !ctx.suppress_emphasis;
    let delimited = effective_bold || effective_italic || run.style.strikethrough;
    let html_wrapped = run.style.superscript || run.style.subscript || run.style.underline;
    // Whether the outermost markup is a `*`/`~~` delimiter that lands against this run's
    // neighbours. Inside a tag or a link's brackets it lands against `>`/`[` instead, which
    // CommonMark always accepts.
    let outer_delimiter = match (&run.revision, &options.revision_handling) {
        (RevisionType::Deleted, RevisionHandling::ShowMarkup) => true,
        (RevisionType::Inserted, RevisionHandling::ShowMarkup) => false,
        _ => delimited && !html_wrapped && run.hyperlink.is_none(),
    };
    // Whitespace follows a run that ends a line, whether the break is written as a marker or
    // as the space `render_runs` adds.
    let after = if ends_a_line(run) { Some('\n') } else { after };

    // OOXML stores the whitespace around a word in the runs themselves — often in a run of
    // its own, or flagged with `xml:space="preserve"`. That whitespace sits *between* runs,
    // not inside the markup wrapping this one: `[label ](url)` puts the space inside the
    // link, and `**bold **` is not emphasis at all, because CommonMark refuses a closing
    // delimiter preceded by whitespace. Punctuation that touches a neighbouring word stays
    // outside a delimiter for the same reason (`32*, s*` is not emphasis either).
    let span = if outer_delimiter {
        emphasis_span(&run.text, before, after)
    } else {
        emphasis_span(&run.text, None, None)
    };
    // A run with nothing to say — empty, whitespace only, or punctuation between words —
    // gets no markup: emphasis or a link wrapped around nothing produces delimiters with no
    // content between them. Its text is still emitted, because it separates its neighbours.
    let Some(span) = span else {
        // A code span has no flanking rule: code that is nothing but punctuation — `()`, `->`
        // — is still code. Only the whitespace around it stays outside.
        let code = run.text.trim();
        if run.style.code && !code.is_empty() {
            let start = run.text.len() - run.text.trim_start().len();
            return format!(
                "{}{}{}{}",
                &run.text[..start],
                markdown::code_span(code, ctx.in_table_cell),
                &run.text[start + code.len()..],
                break_marker(run, options)
            );
        }
        let text = if options.escape_special_chars {
            escape_markdown(&run.text, ctx.in_table_cell)
        } else {
            run.text.clone()
        };
        return format!("{text}{}", break_marker(run, options));
    };
    let escape = |s: &str| {
        if options.escape_special_chars {
            escape_markdown(s, ctx.in_table_cell)
        } else {
            s.to_string()
        }
    };
    let leading = escape(&run.text[..span.start]);
    let trailing = escape(&run.text[span.end..]);
    // Apply formatting, innermost first. Code is written raw — nothing is escaped inside a code
    // span, a backslash there is printed — and fenced instead. Emphasis goes inside any HTML
    // tag, where its delimiters touch `>` and `<` rather than the neighbouring text.
    let mut text = if run.style.code {
        markdown::code_span(&run.text[span], ctx.in_table_cell)
    } else {
        escape(&run.text[span])
    };
    if run.style.strikethrough {
        text = format!("~~{}~~", text);
    }
    if effective_bold && effective_italic {
        text = format!("***{}***", text);
    } else if effective_bold {
        text = format!("**{}**", text);
    } else if effective_italic {
        text = format!("*{}*", text);
    }
    if run.style.superscript {
        text = format!("<sup>{}</sup>", text);
    }
    if run.style.subscript {
        text = format!("<sub>{}</sub>", text);
    }
    if run.style.underline {
        text = format!("<u>{}</u>", text);
    }

    // Handle hyperlinks. The link keeps its brackets whatever `escape_special_chars` says:
    // that option is about how text looks, not about where the link ends.
    if let Some(ref url) = run.hyperlink {
        text = markdown::link(&text, url, None, ctx.in_table_cell);
    }

    // Apply revision markup for ShowMarkup mode
    match (&run.revision, &options.revision_handling) {
        (RevisionType::Deleted, RevisionHandling::ShowMarkup) => {
            // Show deleted text with strikethrough
            text = format!("~~{}~~", text);
        }
        (RevisionType::Inserted, RevisionHandling::ShowMarkup) => {
            // Show inserted text with underline markers (using HTML since Markdown lacks insert markup)
            text = format!("<ins>{}</ins>", text);
        }
        _ => {}
    }

    format!("{leading}{text}{trailing}{}", break_marker(run, options))
}

/// Escape Markdown special characters.
///
/// Context-aware escaping - only escapes when the character could actually
/// trigger markdown formatting.
///
/// - `\` - always escape (escape character)
/// - `` ` `` - always escape (inline code)
/// - `[`, `]` - always escape: text written `[x](y)` would be a link, and a line opening
///   `[x]: y` a link reference definition, which is not printed at all. Footnote references
///   and definitions are written by the renderer from the model, not carried in the text.
/// - `|` - escape only inside table cells (where it is the column delimiter).
///   In regular paragraphs `|` is just a literal character.
/// - `*` - escape unless it stands between two spaces (`a * b`): any other `*` is
///   flanking (CommonMark §6.2) and may open or close emphasis -- `*word*`, `a*b*c`.
/// - `_` - same as `*`, **plus** an extra rule: CommonMark forbids intra-word
///   `_` from opening or closing emphasis, so identifiers like `snake_case`
///   are left intact.
/// - `<` where it can open inline HTML or an autolink, and `&` where it starts a character
///   reference -- through `crate::markdown::escape_html_syntax`.
///
/// Characters NOT escaped (only special in specific contexts):
/// - `()`, `{}` - only special after a link's `]`, which is always escaped
/// - `#` - only special at start of line (headings)
/// - `+`, `-` - only special at start of line (lists) or `---` (rules)
/// - `!` - only special before `[` (images)
/// - `.` - only special in ordered lists at line start (e.g., "1.")
fn escape_markdown(s: &str, in_table_cell: bool) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    // A cell's text is a whole inline context: its `*` and `_` are escaped only when some of
    // them could pair into emphasis (`x*y*z`), not the stars of `0.039***`.
    let cell_pairs = |d: char| markdown::emphasis_can_pair(s, d);

    for (i, &c) in chars.iter().enumerate() {
        match c {
            '\\' | '`' | '[' | ']' => {
                result.push('\\');
                result.push(c);
            }
            '|' => {
                if in_table_cell {
                    result.push('\\');
                }
                result.push(c);
            }
            '*' | '_' => {
                let prev = if i > 0 { Some(chars[i - 1]) } else { None };
                let next = chars.get(i + 1).copied();

                // CommonMark §6.2: a delimiter opens or closes emphasis when it is left- or
                // right-flanking. With whitespace on both sides it is neither, and can pair
                // with nothing. Anything else -- after a space and before a word (`_foo`),
                // between words (`a*b`), next to punctuation -- may pair with a delimiter
                // elsewhere on the line, so it is escaped. The ends of `s` are not
                // whitespace: the text written next to it is not known here.
                let spaced =
                    prev.is_some_and(char::is_whitespace) && next.is_some_and(char::is_whitespace);

                // Underscore-only rule: CommonMark disallows intra-word `_`
                // from opening/closing emphasis, so identifiers like
                // `YESUNG_OMS_backup` never need escaping.
                let intra_word_underscore = c == '_'
                    && prev.is_some_and(|p| p.is_alphanumeric())
                    && next.is_some_and(|n| n.is_alphanumeric());

                let literal = if in_table_cell {
                    !cell_pairs(c)
                } else {
                    spaced || intra_word_underscore
                };
                if literal {
                    result.push(c);
                } else {
                    result.push('\\');
                    result.push(c);
                }
            }
            _ => result.push(c),
        }
    }
    markdown::escape_html_syntax(&result)
}

/// Render a table cell's content with formatting preserved.
/// Multiple paragraphs are joined with `<br>` for inline display.
///
/// Note: Nested tables are NOT rendered here to avoid content duplication.
/// The nested_tables field contains tables that are already structurally
/// separate from cell.content. Rendering both would cause duplication
/// in documents where Word places the same content in both locations.
fn render_cell_content(
    cell: &crate::model::Cell,
    options: &RenderOptions,
    resource_map: &ResourceMap,
    is_header_cell: bool,
) -> String {
    let mut parts = Vec::new();

    for para in &cell.content {
        // Merge adjacent runs with same style (like render_paragraph does)
        let merged_para = para.with_merged_runs();
        let mut para_text = String::new();

        // Suppress uniform bold inside a header cell so the markdown header
        // row doesn't end up like `| **Field** |` when the docx applied a
        // blanket bold to every header cell.
        let suppress_emphasis = is_header_cell
            && options.strip_redundant_emphasis_in_headings
            && all_runs_uniformly_bold(&merged_para);
        let ctx = RunContext {
            in_table_cell: true,
            suppress_emphasis,
        };

        // A cell is one line: its text is flattened, and a thematic break in it would be
        // three hyphens among the words. A page break there is a space, as a heading's is.
        let cell_options;
        let run_options = if options.emit_page_breaks {
            cell_options = RenderOptions {
                emit_page_breaks: false,
                ..options.clone()
            };
            &cell_options
        } else {
            options
        };
        render_runs(&merged_para.runs, run_options, ctx, &mut para_text);

        if !para_text.is_empty() {
            parts.push(para_text);
        }

        // Render inline images from paragraph (like render_paragraph does)
        for image in para.images.iter().filter(|_| options.include_images) {
            let alt = image.alt_text.as_deref().unwrap_or("image");
            let path =
                resolve_image_path(&image.resource_id, resource_map, &options.image_path_prefix);
            parts.push(markdown::image(alt, &path, true));
        }
    }

    // NOTE: nested_tables are intentionally NOT rendered here.
    // They are extracted as separate Table blocks during parsing and should
    // be rendered independently to preserve structure and avoid duplication.
    // See: render_nested_tables_as_blocks() for proper nested table rendering.

    // Join paragraphs with <br> for markdown table cells
    let text = parts.join("<br>");
    let text = if options.preserve_line_breaks {
        text.replace("  \n", "<br>")
    } else {
        text
    };

    // Only replace line breaks - pipes are already escaped by escape_markdown
    // in render_run. Bare CR is treated as a line break too: a leaked CR
    // splits the pipe-table row in most Markdown renderers.
    text.replace("\r\n", "\n").replace(['\n', '\r'], " ")
}

/// Effective alignment for a cell.
///
/// Word stores cell alignment two places: directly on the cell
/// (`<w:tcPr>/<w:jc>`) and on the cell's paragraphs (`<w:pPr>/<w:jc>`).
/// Authors normally set only the latter, so when the explicit cell
/// alignment is `Left` (the default) we fall back to the first paragraph's
/// alignment to recover the visual intent.
fn effective_cell_alignment(cell: &crate::model::Cell) -> CellAlignment {
    if cell.alignment != CellAlignment::Left {
        return cell.alignment;
    }
    if let Some(para) = cell.content.first() {
        return match para.alignment {
            crate::model::TextAlignment::Center => CellAlignment::Center,
            crate::model::TextAlignment::Right => CellAlignment::Right,
            _ => CellAlignment::Left,
        };
    }
    CellAlignment::Left
}

/// Get column alignments from the first data row (or first row if no data rows).
/// Returns a vector of alignments for each column.
/// Column alignments for the separator row, read off the laid-out grid.
///
/// Taken from the grid rather than from a row's cell list so that a cell's alignment
/// lands on the column it actually occupies: a vertical merge from an earlier row
/// shifts a row's cells rightward, and counting cells would put every alignment after
/// it one column early.
fn get_column_alignments(
    table: &Table,
    grid: &[Vec<Option<&Cell>>],
    col_count: usize,
) -> Vec<CellAlignment> {
    // Prefer the first data row (non-header); fall back to the first row.
    let source = table.rows.iter().position(|r| !r.is_header).unwrap_or(0);

    let mut alignments = vec![CellAlignment::Left; col_count];
    if let Some(slots) = grid.get(source) {
        for (col, slot) in slots.iter().enumerate().take(col_count) {
            if let Some(cell) = slot {
                alignments[col] = effective_cell_alignment(cell);
            }
        }
        // A column covered by a merge takes the alignment of the cell covering it —
        // otherwise a centred merged cell gets a left-aligned tail.
        let mut carried = CellAlignment::Left;
        for (col, slot) in slots.iter().enumerate().take(col_count) {
            match slot {
                Some(cell) => carried = effective_cell_alignment(cell),
                None => alignments[col] = carried,
            }
        }
    }

    alignments
}

/// If `table` is a single-row, single-column table whose content is fully
/// emphasized (every non-empty run is bold), render it as a blockquote
/// callout: `> **...**`. Returns `None` when the heuristic does not apply.
fn render_callout_blockquote(
    table: &Table,
    options: &RenderOptions,
    resource_map: &ResourceMap,
) -> Option<String> {
    if table.rows.len() != 1 {
        return None;
    }
    let row = &table.rows[0];
    if row.cells.len() != 1 {
        return None;
    }
    let cell = &row.cells[0];
    if cell.content.is_empty() {
        return None;
    }
    let all_bold = cell.content.iter().all(|p| {
        p.runs
            .iter()
            .all(|r| r.text.trim().is_empty() || r.style.bold)
    });
    let any_text = cell
        .content
        .iter()
        .any(|p| p.runs.iter().any(|r| !r.text.trim().is_empty()));
    if !(all_bold && any_text) {
        return None;
    }

    // Render the cell with bold suppressed, then prefix every line with `> `.
    let inner = render_cell_content(cell, options, resource_map, true);
    let inner = inner.replace("<br>", "\n");
    let mut out = String::new();
    for line in inner.lines() {
        if line.trim().is_empty() {
            out.push_str(">\n");
        } else {
            out.push_str("> **");
            out.push_str(line.trim());
            out.push_str("**\n");
        }
    }
    Some(out)
}

/// Render a table to Markdown, then each table nested in its cells as a block of its own.
///
/// Markdown has no table inside a table cell, so the cell holds its own text and a nested
/// table follows the one that holds it -- however that one is written (pipes, HTML or a
/// callout quote), and depth first.
fn render_table(table: &Table, options: &RenderOptions, resource_map: &ResourceMap) -> String {
    let mut output = render_table_without_nested(table, options, resource_map);
    for row in &table.rows {
        for cell in &row.cells {
            for nested in &cell.nested_tables {
                // A blank line ends the holder's block, an HTML one included.
                if !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push('\n');
                output.push_str(&render_table(nested, options, resource_map));
            }
        }
    }
    output
}

/// Render a table to Markdown without the tables nested in its cells.
fn render_table_without_nested(
    table: &Table,
    options: &RenderOptions,
    resource_map: &ResourceMap,
) -> String {
    if table.is_empty() {
        return String::new();
    }

    // Optional: a 1×1 emphasized table is almost always a callout box,
    // not tabular data. Render it as a blockquote when the user opts in.
    if options.callout_blockquote {
        if let Some(quote) = render_callout_blockquote(table, options, resource_map) {
            return quote;
        }
    }

    // Check if we need HTML fallback
    if table.has_merged_cells() && matches!(options.table_fallback, super::TableFallback::Html) {
        return render_table_html(table);
    }

    let mut output = String::new();

    // Merges are placed on a flat grid first. Markdown has no colspan/rowspan, so a
    // merged cell must be anchored at the column it starts in with the columns it
    // covers left empty — otherwise the row is narrower than the table and every
    // value in it shifts out from under its heading.
    let grid = super::grid::lay_out(table);
    let col_count = grid.first().map_or(0, Vec::len);
    if col_count == 0 {
        return String::new();
    }

    for (i, slots) in grid.iter().enumerate() {
        output.push('|');

        // The first row is always rendered as a header row in markdown,
        // regardless of `row.is_header`. Treat it as a header cell for
        // emphasis-suppression purposes too.
        let is_header_row = i == 0 || table.rows[i].is_header;
        for slot in slots {
            match slot {
                Some(cell) => {
                    let text = render_cell_content(cell, options, resource_map, is_header_row);
                    output.push_str(&format!(" {} |", text));
                }
                // A column covered by a merge, or one this row does not reach.
                None => output.push_str("  |"),
            }
        }
        output.push('\n');

        // Add separator after first row (markdown tables always need header separator)
        // In markdown, the first row is always treated as header regardless of source formatting
        if i == 0 {
            output.push('|');
            // Collect alignments from cells, filling with Left for missing columns
            let alignments = get_column_alignments(table, &grid, col_count);
            for alignment in &alignments {
                let separator = match alignment {
                    CellAlignment::Center => " :---: |",
                    CellAlignment::Right => " ---: |",
                    CellAlignment::Left => " --- |",
                };
                output.push_str(separator);
            }
            output.push('\n');
        }
    }

    output
}

/// Render a table as HTML (for complex layouts).
fn render_table_html(table: &Table) -> String {
    let mut html = String::from("<table>\n");

    for row in &table.rows {
        html.push_str("  <tr>\n");
        for cell in &row.cells {
            let tag = if cell.is_header || row.is_header {
                "th"
            } else {
                "td"
            };
            let mut attrs = String::new();
            if cell.col_span > 1 {
                attrs.push_str(&format!(" colspan=\"{}\"", cell.col_span));
            }
            if cell.row_span > 1 {
                attrs.push_str(&format!(" rowspan=\"{}\"", cell.row_span));
            }
            let text = escape_html(&cell.plain_text());
            html.push_str(&format!("    <{}{}>{}</{}>\n", tag, attrs, text, tag));
        }
        html.push_str("  </tr>\n");
    }

    html.push_str("</table>");
    html
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "../../tests/unit/render/markdown.rs"]
mod tests;
