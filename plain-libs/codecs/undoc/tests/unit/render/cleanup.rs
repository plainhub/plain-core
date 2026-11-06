use super::*;

#[test]
fn test_normalize_unicode() {
    // en-dash (\u{2013}) and em-dash (\u{2014})
    let input = "Hello \u{2013} World \u{2014} Test";
    let result = normalize_unicode(input);
    assert_eq!(result, "Hello - World - Test");
}

#[test]
fn test_normalize_quotes() {
    // smart quotes: \u{201C} left, \u{201D} right double; \u{2018} left, \u{2019} right single
    let input = "\u{201C}Smart quotes\u{201D} and \u{2018}apostrophes\u{2019}";
    let result = normalize_unicode(input);
    assert_eq!(result, "\"Smart quotes\" and 'apostrophes'");
}

#[test]
fn test_remove_pua() {
    let input = "Normal text\u{E001}hidden\u{F000}text";
    let result = remove_private_use_area(input);
    assert_eq!(result, "Normal texthiddentext");
}

#[test]
fn test_clean_lines_page_numbers() {
    let input = "Content here\nPage 1\nMore content\n15";
    let result = clean_lines(input);
    assert!(!result.contains("Page 1"));
    assert!(!result.contains("\n15"));
}

/// A leading dash is a Markdown list marker, so `- 5` is a list item whose text
/// happens to be a number — not page furniture. Page decoration puts a dash on
/// *both* sides, and that is the shape worth matching.
#[test]
fn test_clean_lines_keeps_numeric_list_items() {
    let input = "Intro\n- 5\n- 2026\n— 7\nOutro";
    let result = clean_lines(input);

    assert!(
        result.contains("- 5"),
        "a list item is not a page number: {result}"
    );
    assert!(
        result.contains("- 2026"),
        "a year is not a page number: {result}"
    );
    assert!(
        result.contains("— 7"),
        "an em-dash list marker is a marker: {result}"
    );
}

/// The counterpart: decorated page numbers are still removed. Without this, "stop
/// deleting list items" could be read as "stop detecting page numbers".
#[test]
fn test_clean_lines_still_removes_decorated_page_numbers() {
    let input = "Intro\n- 5 -\n— 7 —\nPage 12\n15\nOutro";
    let result = clean_lines(input);

    assert!(
        !result.contains("- 5 -"),
        "symmetric dashes are page decoration"
    );
    assert!(
        !result.contains("— 7 —"),
        "symmetric em dashes are page decoration"
    );
    assert!(
        !result.contains("Page 12"),
        "a labelled page number is page furniture"
    );
    assert!(
        !result.contains("\n15"),
        "a lone short number is page furniture"
    );
    assert!(result.contains("Intro") && result.contains("Outro"));
}

/// `preserve_frontmatter` has to hold for the whole pipeline, not one stage of it.
/// YAML is whitespace-significant, so a later stage that collapses runs of spaces
/// re-nests the document; one that drops single-character lines can delete a value.
#[test]
fn test_preserve_frontmatter_survives_every_stage() {
    let options = CleanupOptions {
        normalize_strings: true,
        remove_pua: true,
        clean_lines: true,
        filter_structure: true,
        final_normalize: true,
        preserve_frontmatter: true,
    };
    let input =
        "---\ntitle: Test\nnested:\n  key: value\n  list:\n    - one\n---\nBody text\nPage 1";
    let result = clean_text(input, &options);

    assert!(
        result.contains("  key: value"),
        "YAML indentation is significant and must survive: {result}"
    );
    assert!(
        result.contains("    - one"),
        "a nested list keeps its indentation: {result}"
    );
    assert!(
        result.contains("Body text"),
        "the body is still cleaned: {result}"
    );
    assert!(
        !result.contains("Page 1"),
        "cleanup still runs on the body: {result}"
    );
}

#[test]
fn test_preserve_frontmatter() {
    let options = CleanupOptions {
        clean_lines: true,
        preserve_frontmatter: true,
        ..Default::default()
    };
    let input = "---\ntitle: Test\n---\nContent\nPage 1";
    let result = clean_text(input, &options);
    assert!(result.contains("title: Test"));
    assert!(!result.contains("Page 1"));
}

/// Without the option, frontmatter is just text — the stages see it like anything
/// else. Pinned so that detaching it never becomes unconditional by accident.
#[test]
fn test_frontmatter_is_not_preserved_when_not_asked_for() {
    let options = CleanupOptions {
        clean_lines: true,
        preserve_frontmatter: false,
        ..Default::default()
    };
    let input = "---\ntitle: Test\n---\nContent\nPage 1";
    let result = clean_text(input, &options);
    assert!(!result.contains("Page 1"));
}

/// An opening fence with no closing fence is not a frontmatter block, so it must not
/// swallow the document.
#[test]
fn test_unterminated_frontmatter_is_not_detached() {
    let options = CleanupOptions {
        clean_lines: true,
        preserve_frontmatter: true,
        ..Default::default()
    };
    let input = "---\ntitle: Test\nBody text";
    let result = clean_text(input, &options);
    assert!(
        result.contains("Body text"),
        "the body must survive: {result}"
    );
}

#[test]
fn test_filter_structure() {
    let input = "Line 1\n\n\n\nLine 2";
    let result = filter_structure(input);
    assert!(!result.contains("\n\n\n")); // No triple blank lines
}

#[test]
fn test_collapse_blank_lines_basic() {
    let input = "A\n\n\n\nB";
    assert_eq!(collapse_blank_lines(input), "A\n\nB");
}

#[test]
fn test_collapse_blank_lines_idempotent() {
    let input = "A\n\nB\n\nC";
    assert_eq!(collapse_blank_lines(input), input);
}

#[test]
fn test_collapse_blank_lines_whitespace_only() {
    // Lines containing only whitespace count as blank
    let input = "A\n\n   \n\t\nB";
    assert_eq!(collapse_blank_lines(input), "A\n\nB");
}

#[test]
fn test_collapse_blank_lines_preserves_single_blank() {
    let input = "A\n\nB";
    assert_eq!(collapse_blank_lines(input), "A\n\nB");
}

#[test]
fn test_collapse_blank_lines_no_blanks() {
    let input = "A\nB\nC";
    assert_eq!(collapse_blank_lines(input), "A\nB\nC");
}

/// Blank-line collapsing belongs to `final_normalize`, so options built by hand rather
/// than from a preset can now switch it off — where it used to run regardless. Every
/// shipped preset enables `final_normalize`, so preset-configured output is unchanged.
#[test]
fn test_blank_line_collapsing_follows_final_normalize() {
    let input = "A




B";

    let collapsing = CleanupOptions {
        final_normalize: true,
        filter_structure: false,
        ..Default::default()
    };
    assert_eq!(
        clean_text(input, &collapsing),
        "A

B"
    );

    let untouched = CleanupOptions {
        final_normalize: false,
        filter_structure: false,
        ..Default::default()
    };
    assert_eq!(clean_text(input, &untouched), input);
}

#[test]
fn test_final_normalize() {
    let input = "Multiple   spaces   here";
    let result = final_normalize(input);
    assert_eq!(result, "Multiple spaces here");
}

#[test]
fn test_final_normalize_preserves_nested_list_indent() {
    let input = "- top\n  - nested\n    - deeper";
    let result = final_normalize(input);
    assert_eq!(result, "- top\n  - nested\n    - deeper");
}

#[test]
fn test_final_normalize_collapses_interior_only() {
    let input = "  - nested   item   \n";
    let result = final_normalize(input);
    assert_eq!(result, "  - nested item");
}

/// The markdown renderer emits two spaces per nesting level for list items, so a cleanup
/// pass that trims leading whitespace silently flattens every nested list it is given.
#[test]
fn test_clean_text_keeps_nested_list_indent() {
    let options = CleanupOptions {
        normalize_strings: true,
        clean_lines: true,
        filter_structure: true,
        final_normalize: true,
        remove_pua: true,
        preserve_frontmatter: true,
    };

    let input = "- top\n  - nested\n    - deeper";
    assert_eq!(
        clean_text(input, &options),
        "- top\n  - nested\n    - deeper"
    );
}

#[test]
fn test_final_normalize_blank_only_input() {
    assert_eq!(final_normalize("\n   \n\n"), "");
}

#[test]
fn test_clean_text_full() {
    let options = CleanupOptions {
        normalize_strings: true,
        clean_lines: true,
        filter_structure: true,
        final_normalize: true,
        remove_pua: true,
        preserve_frontmatter: true,
    };

    let input = "---\ntitle: Test\n---\n\nHello – World\n\n\n\nPage 1\nContent.";
    let result = clean_text(input, &options);

    assert!(result.contains("Hello - World")); // Normalized dash
    assert!(!result.contains("Page 1")); // Removed page number
    assert!(!result.contains("\n\n\n")); // No excess blank lines
}

#[test]
fn test_detect_mojibake() {
    // Mojibake pattern for em-dash: \u{00E2}\u{20AC}\u{201C}
    let input = "This has \u{00E2}\u{20AC}\u{201C} some issues";
    let issues = detect_mojibake(input);
    assert!(!issues.is_empty());
}
