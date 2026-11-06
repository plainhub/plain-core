use super::*;

/// `text` with `*` around its emphasis span.
fn wrap(text: &str, before: Option<char>, after: Option<char>) -> String {
    emphasis_span(text, before, after).map_or(text.to_string(), |r| {
        format!(
            "{}*{}*{}",
            &text[..r.start],
            &text[r.clone()],
            &text[r.end..]
        )
    })
}

#[test]
fn whitespace_stays_outside() {
    assert_eq!(wrap("  word  ", Some('a'), Some('b')), "  *word*  ");
    assert_eq!(emphasis_span("   ", None, None), None);
    assert_eq!(emphasis_span("", None, None), None);
}

#[test]
fn leading_punctuation_against_a_word_moves_out_with_its_space() {
    assert_eq!(wrap(", s", Some('2'), Some(' ')), ", *s*");
}

#[test]
fn trailing_punctuation_against_a_word_moves_out() {
    assert_eq!(wrap("word.", Some(' '), Some('x')), "*word*.");
    assert_eq!(wrap("word, ", Some(' '), Some('x')), "*word,* ");
}

#[test]
fn punctuation_between_words_alone_has_nothing_to_wrap() {
    assert_eq!(emphasis_span(",", Some('8'), Some('a')), None);
    assert_eq!(emphasis_span(", ", Some('8'), None), None);
}

#[test]
fn punctuation_against_space_punctuation_or_an_edge_stays_inside() {
    assert_eq!(wrap("i.e.,", Some(' '), Some(' ')), "*i.e.,*");
    assert_eq!(wrap("(h)", None, None), "*(h)*");
    assert_eq!(wrap("(h)", Some('('), Some(')')), "*(h)*");
    assert_eq!(
        wrap("Continued pretraining.", None, Some(' ')),
        "*Continued pretraining.*"
    );
}

#[test]
fn a_word_on_either_side_moves_both_ends() {
    assert_eq!(wrap("(h)", Some('Q'), Some('x')), "(*h*)");
}

#[test]
fn an_escape_is_never_split() {
    assert_eq!(wrap(r"\*x\_", Some('a'), Some('b')), r"\**x*\_");
}

#[test]
fn a_backslash_before_a_letter_is_not_an_escape() {
    assert_eq!(wrap(r"\a", Some('b'), None), r"\*a*");
}

#[test]
fn non_ascii_punctuation_and_letters_count() {
    assert_eq!(wrap("“word”", Some('x'), Some('y')), "“*word*”");
    assert_eq!(wrap("단어", Some('가'), Some('나')), "*단어*");
}

/// Every block a line of prose can open, escaped at its first character -- and what
/// opens none left as it is.
#[test]
fn block_starts_are_escaped() {
    for (line, escaped) in [
        ("# a", r"\# a"),
        ("###### a", r"\###### a"),
        ("#", r"\#"),
        ("- a", r"\- a"),
        ("+ a", r"\+ a"),
        ("* a", r"\* a"),
        ("-", r"\-"),
        ("1. a", r"1\. a"),
        ("1) a", r"1\) a"),
        ("2024. 10. 14", r"2024\. 10. 14"),
        ("123456789. a", r"123456789\. a"),
        ("> a", r"\> a"),
        (">a", r"\>a"),
        ("```", r"\```"),
        ("~~~ rust", r"\~~~ rust"),
        ("---", r"\---"),
        ("* * *", r"\* * *"),
        ("___", r"\___"),
        ("===", r"\==="),
        ("   # a", r"   \# a"),
    ] {
        assert_eq!(escape_block_starts(line), escaped, "{line:?}");
    }
    for line in [
        "#a",
        "####### a",
        "-a",
        "*a*",
        "1.5 litres",
        "1234567890. a",
        "a. b",
        "__init__",
        "**",
        "= a",
        "``",
        "    # a",
        r"\* a",
        "",
    ] {
        assert_eq!(escape_block_starts(line), line, "{line:?}");
    }
}

/// A delimiter pairs only with a later one that can close; a lone opener or closer, or
/// underscores inside words, make no emphasis.
#[test]
fn emphasis_pairs_only_opener_then_closer() {
    for (text, delimiter, pairs) in [
        ("0.039***", '*', false),
        ("*8 ml", '*', false),
        ("p < .05*", '*', false),
        ("a * b * c", '*', false),
        ("x*y*z", '*', true),
        ("*bold* text", '*', true),
        ("**strong**", '*', true),
        ("a _foo_ b", '_', true),
        ("snake_case_name", '_', false),
        ("_init", '_', false),
        ("(*) and (*)", '*', true),
    ] {
        assert_eq!(emphasis_can_pair(text, delimiter), pairs, "{text:?}");
    }
}

/// What can open inline HTML or a character reference is escaped; what cannot is left.
#[test]
fn html_syntax_is_escaped_where_it_can_open() {
    for (text, escaped) in [
        ("Vec<String>", r"Vec\<String>"),
        ("</div>", r"\</div>"),
        ("<!-- c -->", r"\<!-- c -->"),
        ("<?php", r"\<?php"),
        ("<https://example.com>", r"\<https://example.com>"),
        ("ends with <", r"ends with \<"),
        ("&amp; &#38; &#x26;", r"\&amp; \&#38; \&#x26;"),
    ] {
        assert_eq!(escape_html_syntax(text), escaped, "{text:?}");
    }
    for text in [
        "1 < 2",
        "a <= b",
        "<3",
        "<< b",
        "R&D",
        "a & b",
        "&notareference",
        "& ;",
    ] {
        assert_eq!(escape_html_syntax(text), text, "{text:?}");
    }
}

/// Every line is a line start, not only the first: a paragraph with a hard break.
#[test]
fn every_line_of_the_text_is_checked() {
    assert_eq!(
        escape_block_starts("first  \n# second\nthird"),
        "first  \n\\# second\nthird"
    );
}
