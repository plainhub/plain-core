use super::*;

fn resolve(name: &str) -> String {
    resolve_general_ref(&BytesRef::new(name))
}

#[test]
fn resolves_predefined_entities() {
    assert_eq!(resolve("amp"), "&");
    assert_eq!(resolve("lt"), "<");
    assert_eq!(resolve("gt"), ">");
    assert_eq!(resolve("quot"), "\"");
    assert_eq!(resolve("apos"), "'");
}

#[test]
fn resolves_numeric_character_references() {
    assert_eq!(resolve("#65"), "A"); // decimal
    assert_eq!(resolve("#x41"), "A"); // lower-case hex
    assert_eq!(resolve("#X41"), "A"); // upper-case hex
    assert_eq!(resolve("#128512"), "\u{1F600}"); // astral plane
    assert_eq!(resolve("#44032"), "가"); // multi-byte (U+AC00)
}

#[test]
fn unknown_entity_is_preserved_literally() {
    assert_eq!(resolve("nbsp"), "&nbsp;");
    assert_eq!(resolve("custom"), "&custom;");
}

#[test]
fn empty_reference_is_preserved_literally() {
    // A `&;` token arrives from the reader as GeneralRef("") — preserve it
    // verbatim rather than dropping it (matches 0.37 lenient behavior).
    assert_eq!(resolve(""), "&;");
}

#[test]
fn malformed_references_degrade_without_panicking() {
    // Adversarial inputs must never panic — the parser's robustness goal.
    assert_eq!(resolve("#"), ""); // empty numeric ref
    assert_eq!(resolve("#x"), ""); // empty hex ref
    assert_eq!(resolve("#xZZ"), ""); // non-hex digits
    assert_eq!(resolve("#x110000"), ""); // above the Unicode maximum
    assert_eq!(resolve("#xD800"), ""); // lone surrogate (not a scalar value)
    assert_eq!(resolve("#xFFFFFFFFFFFF"), ""); // overflows u32 parse
}

#[test]
fn lossy_normalizes_literal_carriage_returns() {
    // quick-xml does not perform XML §2.11 EOL normalization, so literal
    // CR/CRLF from non-conforming producers must be normalized here. Bare
    // CR breaks Markdown pipe tables (issue #7).
    let text = BytesText::from_escaped("one\rtwo\r\nthree");
    assert_eq!(decode_text_lossy(&text), "one\ntwo\nthree");
}

#[test]
fn strict_normalizes_literal_carriage_returns() {
    let text = BytesText::from_escaped("a\r\nb\rc");
    assert_eq!(decode_text_strict(&text, "test").unwrap(), "a\nb\nc");
}

#[test]
fn normalize_is_idempotent_and_borrows_without_cr() {
    let clean = normalize_line_endings(Cow::Borrowed("no carriage returns"));
    assert!(matches!(clean, Cow::Borrowed(_)));
    // Double application collapses nothing further.
    let once = normalize_line_endings(Cow::Borrowed("a\r\nb"));
    let twice = normalize_line_endings(once.clone());
    assert_eq!(once, twice);
    assert_eq!(once, "a\nb");
}
