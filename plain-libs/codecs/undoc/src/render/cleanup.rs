//! Text cleanup and normalization utilities.
//!
//! This module provides text cleaning functionality optimized for
//! LLM training data preparation.

use super::options::CleanupOptions;

/// Clean text according to the provided options.
pub fn clean_text(text: &str, options: &CleanupOptions) -> String {
    // Frontmatter is detached before any stage runs and reattached afterwards, rather
    // than being skipped stage by stage. YAML is whitespace-significant and its values
    // are data, not prose: collapsing runs of spaces re-nests the document, and dropping
    // single-character lines deletes a value. A stage that does not know about
    // frontmatter cannot be trusted to leave it alone, so none of them see it.
    let (frontmatter, body) = if options.preserve_frontmatter {
        split_frontmatter(text)
    } else {
        (None, text)
    };

    let mut result = body.to_string();

    if options.normalize_strings {
        result = normalize_unicode(&result);
    }

    if options.remove_pua {
        result = remove_private_use_area(&result);
    }

    if options.clean_lines {
        result = clean_lines(&result);
    }

    if options.filter_structure {
        result = filter_structure(&result);
    }

    if options.final_normalize {
        result = final_normalize(&result);
        // Runs of blank lines are whitespace like any other, so collapsing them belongs to
        // whitespace normalization — and therefore to the option that governs it. It used to
        // run outside the cleanup options entirely, which made `cleanup: None` mean
        // "almost no post-processing" and gave Markdown and text output different policies.
        result = collapse_blank_lines(&result);
    }

    match frontmatter {
        Some(frontmatter) => format!("{frontmatter}\n{result}"),
        None => result,
    }
}

/// Split a leading YAML frontmatter block off the text, if there is one.
///
/// Returns the block without its trailing newline and the remaining body. A document
/// with no frontmatter — or with an unterminated opening fence, which is not a block —
/// yields `None` and the original text.
fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let Some(rest) = text.strip_prefix("---\n") else {
        return (None, text);
    };

    // The closing fence is a line of its own, so search from a line boundary.
    let Some(offset) = rest.find("\n---\n") else {
        return (None, text);
    };

    let end = "---\n".len() + offset + "\n---".len();
    (Some(&text[..end]), text[end..].trim_start_matches('\n'))
}

/// Normalize Unicode strings to NFC form and standardize common elements.
fn normalize_unicode(text: &str) -> String {
    let normalized = plain_libs::normalization::nfc_text(text);

    // Standardize bullets and dashes
    normalized
        // Various bullet characters
        .replace(['•', '◦', '▪', '▫', '●', '○', '■', '□'], "•")
        // Various dashes (en-dash, em-dash, minus sign, figure dash)
        .replace(['\u{2013}', '\u{2014}', '\u{2212}', '\u{2012}'], "-")
        // Various single quotes (left single, right single)
        .replace(['\u{2018}', '\u{2019}'], "'")
        // Various double quotes (left, right, low-9, left guillemet, right guillemet)
        .replace(
            ['\u{201C}', '\u{201D}', '\u{201E}', '\u{00AB}', '\u{00BB}'],
            "\"",
        )
        // Various spaces (non-breaking, en, em, thin, hair, narrow no-break)
        .replace(
            [
                '\u{00A0}', '\u{2002}', '\u{2003}', '\u{2009}', '\u{200A}', '\u{202F}',
            ],
            " ",
        )
        // Zero-width characters (remove: zero-width space, non-joiner, joiner, BOM)
        .replace(['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'], "")
}

/// Remove Private Use Area (PUA) characters.
fn remove_private_use_area(text: &str) -> String {
    text.chars()
        .filter(|c| {
            let code = *c as u32;
            // Private Use Area ranges:
            // U+E000 - U+F8FF (BMP PUA)
            // U+F0000 - U+FFFFD (Supplementary PUA-A)
            // U+100000 - U+10FFFD (Supplementary PUA-B)
            !((0xE000..=0xF8FF).contains(&code)
                || (0xF0000..=0xFFFFD).contains(&code)
                || (0x100000..=0x10FFFD).contains(&code))
        })
        .collect()
}

/// Clean lines - remove headers, footers, page numbers, TOC markers.
///
/// Frontmatter never reaches here: [`clean_text`] detaches it before any stage runs.
fn clean_lines(text: &str) -> String {
    let mut result = Vec::new();

    for line in text.lines() {
        // Skip likely header/footer patterns
        if should_skip_line(line) {
            continue;
        }

        result.push(line);
    }

    result.join("\n")
}

/// Check if a line should be skipped (header, footer, page number, etc.).
fn should_skip_line(line: &str) -> bool {
    let trimmed = line.trim();

    // Empty lines are not skipped
    if trimmed.is_empty() {
        return false;
    }

    // Page number patterns
    if is_page_number(trimmed) {
        return true;
    }

    // Common header/footer patterns
    if is_header_footer(trimmed) {
        return true;
    }

    // TOC marker patterns
    if is_toc_marker(trimmed) {
        return true;
    }

    false
}

/// Check if line is a page number.
///
/// A dash on the left is not evidence of one: in Markdown that is a list marker, so
/// `- 5` is a list item whose text happens to be a number. Page decoration puts a dash
/// on *both* sides (`- 5 -`), and that symmetry is what distinguishes the two.
fn is_page_number(line: &str) -> bool {
    fn is_number(s: &str) -> bool {
        let s = s.trim();
        !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
    }

    // A labelled page number needs no decoration to be unambiguous.
    for label in ["Page ", "page "] {
        if let Some(rest) = line.strip_prefix(label) {
            if is_number(rest) {
                return true;
            }
        }
    }

    for dash in ['-', '—'] {
        // Symmetric decoration: "- 5 -". Asymmetric on the left is a list marker.
        if let Some(inner) = line
            .strip_prefix(dash)
            .and_then(|rest| rest.strip_suffix(dash))
        {
            if is_number(inner) {
                return true;
            }
        }

        // Trailing decoration alone ("5 -") carries no list-marker meaning.
        if let Some(rest) = line.strip_suffix(dash) {
            if is_number(rest) {
                return true;
            }
        }
    }

    // Just a number alone (potential page number)
    if line.len() <= 5 && is_number(line) {
        return true;
    }

    false
}

/// Check if line is a common header/footer.
fn is_header_footer(line: &str) -> bool {
    let lower = line.to_lowercase();

    // Common footer phrases
    let footer_patterns = [
        "all rights reserved",
        "confidential",
        "proprietary",
        "copyright ©",
        "copyright (c)",
        "© ",
        "(c) ",
    ];

    for pattern in footer_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }

    false
}

/// Check if line is a TOC marker.
fn is_toc_marker(line: &str) -> bool {
    let lower = line.to_lowercase();

    // TOC patterns - lines with lots of dots (leader dots)
    if line.contains("...") || line.contains("…") {
        // If it has dots followed by a number, likely TOC entry
        let dot_count = line.chars().filter(|c| *c == '.').count();
        if dot_count > 3 {
            return true;
        }
    }

    // Explicit TOC headers
    if lower == "table of contents" || lower == "contents" {
        return true;
    }

    false
}

/// Collapse 3+ consecutive newlines into a single blank line (`\n\n`).
///
/// This is a lossless normalization: per CommonMark, two or more blank lines
/// render identically to one blank line, so the output is semantically
/// equivalent to the input but more compact.
pub(crate) fn collapse_blank_lines(text: &str) -> String {
    let mut result = Vec::new();
    let mut prev_blank = false;

    for line in text.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        result.push(line);
        prev_blank = is_blank;
    }

    result.join("\n")
}

/// Filter structural elements - remove empty paragraphs, orphaned elements.
fn filter_structure(text: &str) -> String {
    let stripped: String = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.len() != 1 {
                return true;
            }
            !trimmed
                .chars()
                .next()
                .is_some_and(|c| matches!(c, '|' | '-' | '_' | '=' | '*' | '#' | '~'))
        })
        .collect::<Vec<_>>()
        .join("\n");

    collapse_blank_lines(&stripped)
}

/// Final whitespace normalization.
///
/// Leading whitespace is preserved verbatim: in CommonMark it is the only way a document
/// expresses list nesting, so collapsing it flattens the hierarchy. Only trailing
/// whitespace and runs *inside* a line are normalized.
fn final_normalize(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    for line in text.lines() {
        let without_trailing = line.trim_end();
        let content = without_trailing.trim_start();

        if content.is_empty() {
            lines.push(String::new());
            continue;
        }

        let indent = &without_trailing[..without_trailing.len() - content.len()];
        let mut normalized_line = String::with_capacity(without_trailing.len());
        normalized_line.push_str(indent);

        let mut prev_space = false;
        for c in content.chars() {
            if c.is_whitespace() {
                if !prev_space {
                    normalized_line.push(' ');
                    prev_space = true;
                }
            } else {
                normalized_line.push(c);
                prev_space = false;
            }
        }

        lines.push(normalized_line);
    }

    // Drop leading/trailing blank lines. Trimming the joined string instead would strip the
    // indentation of the first line, which is exactly what this function must not do.
    let Some(end) = lines.iter().rposition(|line| !line.is_empty()) else {
        return String::new();
    };
    let start = lines
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or_default();

    lines[start..=end].join("\n")
}

/// Detect potential mojibake patterns.
///
/// This reports what it finds and changes nothing: the patterns it recognises are
/// ambiguous with legitimate text, so repairing them automatically would corrupt documents
/// that were never mis-encoded. It is a diagnostic a caller invokes deliberately, which is
/// why it is not a cleanup option — an option implies the pipeline acts on the result.
pub fn detect_mojibake(text: &str) -> Vec<(usize, String)> {
    let mut issues = Vec::new();

    // Common mojibake patterns (UTF-8 decoded as Windows-1252, etc.)
    // These are byte sequences that result from mis-encoding
    let patterns: &[(&str, &str)] = &[
        ("\u{00E2}\u{20AC}\u{201C}", "em-dash"),
        ("\u{00E2}\u{20AC}\u{2122}", "apostrophe"),
        ("\u{00E2}\u{20AC}\u{0153}", "left quote"),
        ("\u{00C3}\u{00A9}", "e-acute"),
        ("\u{00C3}\u{00A8}", "e-grave"),
        ("\u{00C3}\u{00A0}", "a-grave"),
        ("\u{00C3}\u{00A2}", "a-circumflex"),
        ("\u{00C2}\u{00A0}", "non-breaking space"),
        ("\u{00C3}\u{00A7}", "c-cedilla"),
    ];

    for (i, line) in text.lines().enumerate() {
        for (pattern, desc) in patterns {
            if line.contains(pattern) {
                issues.push((i + 1, format!("Possible mojibake: {} ({})", pattern, desc)));
            }
        }
    }

    issues
}

#[cfg(test)]
#[path = "../../tests/unit/render/cleanup.rs"]
mod tests;
