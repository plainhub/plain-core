//! CommonMark syntax facts every Markdown writer in the family needs.
//!
//! Each library renders its own document model, and how it styles a run is its own business.
//! *Where* an emphasis delimiter may stand, and how text the document supplies — a picture's
//! description, a link's target — has to be written so that it stays inside the construct it
//! belongs to, are not: they are properties of CommonMark, and a writer that gets them wrong
//! prints its syntax as text. This module answers those questions once so the writers cannot
//! drift on them.
//!
//! `std`-only, like the rest of the crate root.

use std::ops::Range;

/// The part of a styled run that emphasis delimiters (`*`, `**`, `~~`) can wrap, given the
/// characters the run lands between, or `None` when no part can.
///
/// `before` is the last character already written, `after` the first one the next piece will
/// write (`None` at either end of the inline content). The text outside the returned range is
/// written as-is, outside the delimiters.
///
/// CommonMark reads a delimiter as emphasis only when it *flanks* the text (§6.2): it may not
/// touch whitespace on its inside, and when its inside is punctuation its outside must be
/// whitespace, punctuation, or the end of the line. So:
///
/// - leading and trailing whitespace always stays outside — `**word **` is not emphasis;
/// - leading or trailing punctuation stays outside too, with any whitespace behind it, when
///   the run touches a letter or digit on that side — an italic `", s"` after `32` written as
///   `32*, s*` shows its asterisks, written as `32, *s*` it does not;
/// - a run that is nothing but whitespace, or nothing but punctuation between words, has
///   nothing to wrap.
///
/// Punctuation that does not touch a word stays inside (`*i.e.,*`, `*(h)*`), so a run is not
/// reshaped unless it has to be.
///
/// `text` may already be escaped: a backslash escape (`\*`) is one unit and is never split
/// across a delimiter. Only escapes of ASCII punctuation are units, as in CommonMark.
///
/// ```
/// use crate::markdown::emphasis_span;
///
/// let wrap = |text: &str, before, after| {
///     emphasis_span(text, before, after).map_or(text.to_string(), |r| {
///         format!("{}*{}*{}", &text[..r.start], &text[r.clone()], &text[r.end..])
///     })
/// };
/// assert_eq!(wrap(", s", Some('2'), Some(' ')), ", *s*");
/// assert_eq!(wrap("word ", None, Some('x')), "*word* ");
/// assert_eq!(wrap(",", Some('8'), Some('a')), ",");
/// assert_eq!(wrap("(h)", Some(' '), None), "*(h)*");
/// ```
pub fn emphasis_span(
    text: &str,
    before: Option<char>,
    after: Option<char>,
) -> Option<Range<usize>> {
    let mut start = text.len() - text.trim_start().len();
    let mut end = text.trim_end().len();
    if start >= end {
        return None;
    }
    let touches_a_word = |c: Option<char>| c.is_some_and(|c| !is_flanking_neutral(c));
    if start == 0 && touches_a_word(before) {
        start = units(text, start, end)
            .find(|&(i, j)| !unit_is_neutral(&text[i..j]))
            .map_or(end, |(i, _)| i);
    }
    if end == text.len() && touches_a_word(after) {
        end = units(text, start, end)
            .filter(|&(i, j)| !unit_is_neutral(&text[i..j]))
            .last()
            .map_or(start, |(_, j)| j);
    }
    (start < end).then_some(start..end)
}

/// A picture as an inline image: `![alt](destination)`.
///
/// The alt text is data from the document — whatever a person typed into a description box:
/// several lines, blank lines, brackets. Written verbatim, a blank line ends the paragraph and
/// the image reads back as literal text plus a stray paragraph, and an unbalanced `]` ends the
/// link text early. So the alt text is flattened to one line — every run of whitespace becomes
/// one space, the ends trimmed — and the characters that are syntax inside link text are
/// escaped: `\`, `[`, `]`, `` ` `` (a code span would swallow the closing bracket), `<` (raw
/// HTML or an autolink), and an `&` that would start a character reference (`&amp;` would read
/// back as `&`). Inside a table cell `|` is escaped too, since it would end the cell. `*` and
/// `_` are left alone: unmatched they are literal, matched they only style the text, and
/// neither can end the image.
///
/// The destination is written by [`link_destination`].
///
/// ```
/// use crate::markdown::image;
///
/// assert_eq!(
///     image("A view\n\nof the harbour", "img/1.png", false),
///     "![A view of the harbour](img/1.png)"
/// );
/// assert_eq!(image("a [draft]", "my dir/a.png", false), r"![a \[draft\]](<my dir/a.png>)");
/// assert_eq!(image("x | y", "a.png", true), r"![x \| y](a.png)");
/// ```
pub fn image(alt: &str, destination: &str, in_table_cell: bool) -> String {
    let mut out = String::with_capacity(alt.len() + destination.len() + 5);
    out.push_str("![");
    for (n, word) in alt.split_whitespace().enumerate() {
        if n > 0 {
            out.push(' ');
        }
        for (i, c) in word.char_indices() {
            match c {
                '\\' | '[' | ']' | '`' | '<' => {
                    out.push('\\');
                    out.push(c);
                }
                '&' if starts_character_reference(&word[i + 1..]) => out.push_str("\\&"),
                '|' if in_table_cell => out.push_str("\\|"),
                _ => out.push(c),
            }
        }
    }
    out.push_str("](");
    out.push_str(&link_destination(destination, in_table_cell));
    out.push(')');
    out
}

/// A code span holding `text` exactly as given: CommonMark reads it back as `text`, every line
/// ending in it read as one space.
///
/// Nothing is escaped inside a code span — a backslash there is printed (CommonMark §6.1), so
/// the text is written as it is and the span is fenced instead:
///
/// - the fence is one backtick longer than the longest run of backticks in the text, so no run
///   inside can close it;
/// - one space pads each side when the text starts or ends with a backtick (it would join the
///   fence) or both starts and ends with a space (a reader strips one from each side) — the
///   reader strips the padding again;
/// - a line ending becomes a space, so a blank line cannot end the paragraph mid-span;
/// - inside a table cell `|` is escaped — a GFM table splits its cells before it reads code
///   spans, and reads `\|` back as `|` inside them.
///
/// Empty text has no code span and gives an empty string.
///
/// ```
/// use crate::markdown::code_span;
///
/// assert_eq!(code_span(r"C:\dir", false), r"`C:\dir`");
/// assert_eq!(code_span("a`b", false), "``a`b``");
/// assert_eq!(code_span("`x`", false), "`` `x` ``");
/// assert_eq!(code_span(" a ", false), "`  a  `");
/// assert_eq!(code_span("a | b", true), r"`a \| b`");
/// assert_eq!(code_span("", false), "");
/// ```
pub fn code_span(text: &str, in_table_cell: bool) -> String {
    if text.is_empty() {
        return String::new();
    }
    let text = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
    let longest_run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest_run + 1);
    let pad = text.starts_with('`')
        || text.ends_with('`')
        || (text.starts_with(' ')
            && text.ends_with(' ')
            && !text.trim_start_matches(' ').is_empty());
    let pad = if pad { " " } else { "" };
    let body = if in_table_cell {
        text.replace('|', "\\|")
    } else {
        text
    };
    format!("{fence}{pad}{body}{pad}{fence}")
}

/// An inline link: `[label](destination)`, or `[label](destination "title")`.
///
/// `label` is inline Markdown the writer has already rendered — its text escaped and styled
/// the writer's own way (`**bold**`, `<u>…</u>`, a code span). What this adds is what keeps
/// that Markdown *inside the link*, which no display setting may turn off:
///
/// - an unescaped `[` or `]` is escaped — an unbalanced `]` ends the link text early, and a
///   `[x](y)` inside it would be a second link, which CommonMark does not nest;
/// - a line ending becomes a space, so a blank line cannot end the paragraph mid-link;
/// - a backslash at the very end is doubled, since it would otherwise escape the closing `]`;
/// - inside a table cell, an unescaped `|` is escaped, code spans included — a GFM table
///   splits its cells before it reads anything else.
///
/// Code spans are left as they are (apart from that `|`): a code span binds more tightly than
/// the link's brackets, so a `]` inside one does not end the link, and a backslash inside one
/// would be printed. Backslash escapes the label already carries (`\*`, `\[`) are kept as
/// they are. The label is not expected to hold a link, an image, an autolink or raw HTML with
/// a bracket in it.
///
/// The destination is written by [`link_destination`]. The title, when there is one and it is
/// not empty, is written in double quotes: `"` is escaped, a backslash that would escape the
/// character after it is doubled, an `&` that would start a character reference is escaped,
/// a line ending becomes a space, and inside a table cell `|` is escaped.
///
/// ```
/// use crate::markdown::link;
///
/// assert_eq!(link("see [3]", "https://example.com", None, false), r"[see \[3\]](https://example.com)");
/// assert_eq!(link("**bold** `a]b`", "u", None, false), "[**bold** `a]b`](u)");
/// assert_eq!(link("x", "u", Some(r#"say "hi""#), false), r#"[x](u "say \"hi\"")"#);
/// assert_eq!(link("a | b", "u", None, true), r"[a \| b](u)");
/// ```
pub fn link(label: &str, destination: &str, title: Option<&str>, in_table_cell: bool) -> String {
    let mut out = String::with_capacity(label.len() + destination.len() + 4);
    out.push('[');
    push_link_label(&mut out, label, in_table_cell);
    out.push_str("](");
    out.push_str(&link_destination(destination, in_table_cell));
    if let Some(title) = title.filter(|t| !t.is_empty()) {
        out.push_str(" \"");
        let mut chars = title.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' if chars.peek().is_none_or(|&(_, n)| n.is_ascii_punctuation()) => {
                    out.push_str("\\\\")
                }
                '"' => out.push_str("\\\""),
                '&' if starts_character_reference(&title[i + 1..]) => out.push_str("\\&"),
                '|' if in_table_cell => out.push_str("\\|"),
                '\n' | '\r' => out.push(' '),
                _ => out.push(c),
            }
        }
        out.push('"');
    }
    out.push(')');
    out
}

/// Append `label` as link text — see [`link`] for what is escaped and what is kept.
fn push_link_label(out: &mut String, label: &str, in_table_cell: bool) {
    let chars: Vec<char> = label.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' => match chars.get(i + 1) {
                // An escape the label already carries: one unit, kept.
                Some(&n) if n.is_ascii_punctuation() => {
                    out.push('\\');
                    out.push(n);
                    i += 2;
                    continue;
                }
                None => out.push_str("\\\\"),
                Some(_) => out.push('\\'),
            },
            '`' => {
                let run = chars[i..].iter().take_while(|&&b| b == '`').count();
                if let Some(close) = closing_backticks(&chars, i + run, run) {
                    // A code span, written as it is: no escapes inside, but a GFM cell
                    // still splits on its `|`.
                    for (k, &s) in chars[i..close + run].iter().enumerate() {
                        match s {
                            '\n' | '\r' => out.push(' '),
                            '|' if in_table_cell && (k == 0 || chars[i + k - 1] != '\\') => {
                                out.push_str("\\|")
                            }
                            _ => out.push(s),
                        }
                    }
                    i = close + run;
                } else {
                    // No closing run of the same length: the backticks are literal.
                    out.extend(std::iter::repeat_n('`', run));
                    i += run;
                }
                continue;
            }
            '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            '|' if in_table_cell => out.push_str("\\|"),
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
        i += 1;
    }
}

/// Where the backtick run of exactly `len` that closes a code span opened before `from`
/// starts, if there is one.
fn closing_backticks(chars: &[char], from: usize, len: usize) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '`' {
            let run = chars[j..].iter().take_while(|&&b| b == '`').count();
            if run == len {
                return Some(j);
            }
            j += run;
        } else {
            j += 1;
        }
    }
    None
}

/// A link or image destination, written so that CommonMark reads back exactly `url` (§6.3).
///
/// The bare form is used when it can carry the URL: no spaces, no ASCII control characters,
/// no `<` or `>`, and parentheses that balance — an unbalanced `)` would end the destination
/// early, a `(` would leave it open. Anything else is written in the pointy-bracket form
/// `<...>`, which takes spaces and any parentheses. A line ending cannot stand in either form;
/// it is percent-encoded (`%0A`, `%0D`), the one place the URL is changed rather than
/// escaped. In both forms a backslash that would otherwise escape the character after it — an
/// ASCII punctuation character, or the end of the destination — is doubled; other backslashes
/// (`C:\dir`) are literal and stay as they are. An `&` that would start a character reference
/// is escaped, so `?a=1&amp;b` stays as written. Inside a table cell `|` is escaped, since a
/// GFM table splits its cells before it reads links. An empty URL is `<>`.
///
/// A link that is not in a table cell passes `in_table_cell: false`.
///
/// ```
/// use crate::markdown::link_destination;
///
/// assert_eq!(link_destination("https://example.com/a(b)", false), "https://example.com/a(b)");
/// assert_eq!(link_destination("my folder/file.png", false), "<my folder/file.png>");
/// assert_eq!(link_destination("notes).txt", false), "<notes).txt>");
/// assert_eq!(link_destination("a<b>c", false), r"<a\<b\>c>");
/// ```
pub fn link_destination(url: &str, in_table_cell: bool) -> String {
    let bare = !url.is_empty()
        && !url
            .chars()
            .any(|c| c == ' ' || c == '<' || c == '>' || c.is_ascii_control())
        && parentheses_balance(url);
    let mut out = String::with_capacity(url.len() + 2);
    if !bare {
        out.push('<');
    }
    let mut chars = url.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            // Judged on what is written next: a line ending goes out as `%`.
            '\\' if chars
                .peek()
                .is_none_or(|&(_, n)| n.is_ascii_punctuation() || n == '\n' || n == '\r') =>
            {
                out.push_str("\\\\")
            }
            '&' if starts_character_reference(&url[i + 1..]) => out.push_str("\\&"),
            '<' | '>' => {
                out.push('\\');
                out.push(c);
            }
            '|' if in_table_cell => out.push_str("\\|"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            _ => out.push(c),
        }
    }
    if !bare {
        out.push('>');
    }
    out
}

/// Whether the text after an `&` makes it a character reference (§6.2) — `name;`, `#123;` or
/// `#x1F;` — which CommonMark would replace by the character it names.
/// `text` with whatever would open a block at the start of one of its lines escaped, so that
/// prose written into a paragraph or a list item reads back as that prose.
///
/// A writer puts its own block markers in front of the text — `# ` for a heading it means,
/// `1. ` for a list it means. Text the document supplies is not supposed to add any, but a
/// paragraph whose text starts `1. `, `* `, `# ` or `> ` is, written as is, a list item, a
/// heading or a quote (CommonMark §4–5). At the start of each line, after at most three
/// spaces, this escapes the first character of:
///
/// - an ATX heading opener: one to six `#` followed by a space, a tab or the line's end;
/// - a bullet list marker: `-`, `+` or `*` followed by a space, a tab or the line's end;
/// - an ordered list marker: one to nine digits and `.` or `)`, followed likewise (the
///   delimiter is escaped: `1\.`);
/// - a block quote marker: `>`;
/// - a code fence: three or more `` ` `` or `~`;
/// - a thematic break or setext heading underline: a line of `-`, `*`, `_` (spaces between
///   allowed) or of `=` alone.
///
/// A line indented four spaces or more is left alone — a paragraph continues through it, and
/// a writer has already removed indentation from a paragraph's first line. A character that
/// is already escaped (`\*`) opens nothing and is left as it is. Not for table cells, where
/// no block can start.
///
/// ```
/// use crate::markdown::escape_block_starts;
///
/// assert_eq!(escape_block_starts("1. A sentence that starts with a number"), r"1\. A sentence that starts with a number");
/// assert_eq!(escape_block_starts("# not a heading"), r"\# not a heading");
/// assert_eq!(escape_block_starts("* note\n> quoted"), "\\* note\n\\> quoted");
/// assert_eq!(escape_block_starts("#hashtag, 2024. 10. 14"), "#hashtag, 2024. 10. 14");
/// ```
pub fn escape_block_starts(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for (n, line) in text.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let indent = line.len() - line.trim_start_matches(' ').len();
        match block_marker_at(&line[indent.min(line.len())..]).filter(|_| indent <= 3) {
            Some(at) => {
                let at = indent + at;
                out.push_str(&line[..at]);
                out.push('\\');
                out.push_str(&line[at..]);
            }
            None => out.push_str(line),
        }
    }
    out
}

/// `text` with what a Markdown reader would take for HTML escaped: a `<` that can open a tag,
/// a comment, a declaration, a processing instruction or an autolink, and a `&` that starts
/// a character reference.
///
/// Text written into Markdown is read for inline HTML (CommonMark §6.6–6.7): `Vec<String>`
/// shows as `Vec` with an unknown tag swallowed, `a <b>bold</b>` turns bold, `AT&amp;T`
/// shows as `AT&T`. A `<` is escaped unless the character after it cannot start any of those
/// — a space, a digit, `=`, … — and a `<` that ends `text` is escaped too, since the next
/// piece written after it may start with a letter. A `&` is escaped only where a character
/// reference starts (`&name;`, `&#…;`), so `R&D` and `a & b` are left alone.
///
/// `text` may already be escaped for Markdown (`\\` for a backslash); apply this after that.
///
/// ```
/// use crate::markdown::escape_html_syntax;
///
/// assert_eq!(escape_html_syntax("Vec<String>"), r"Vec\<String>");
/// assert_eq!(escape_html_syntax("a <b>bold</b>"), r"a \<b>bold\</b>");
/// assert_eq!(escape_html_syntax("AT&amp;T, R&D, 1 < 2"), r"AT\&amp;T, R&D, 1 < 2");
/// ```
pub fn escape_html_syntax(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for (i, c) in text.char_indices() {
        let rest = &text[i + c.len_utf8()..];
        let opens = match c {
            '<' => rest
                .chars()
                .next()
                .is_none_or(|n| n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?')),
            '&' => starts_character_reference(rest),
            _ => false,
        };
        if opens {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Whether the `delimiter` (`*` or `_`) characters of `text` can make emphasis among
/// themselves: a run of them that can open (CommonMark §6.2: left-flanking, and for `_` not
/// inside a word) followed somewhere later by a run that can close.
///
/// `text` is a complete inline context — a table cell, a link label — so when no run can
/// pair, every delimiter in it is read as itself and none needs escaping: the significance
/// stars of `0.039***`, a lone `*8 ml`. Without a later closer an opener opens nothing.
/// The answer errs toward `true`: the rule of three (§6.2, rules 9–10) is not applied, so a
/// pair it would forbid still counts as one.
///
/// ```
/// use crate::markdown::emphasis_can_pair;
///
/// assert!(!emphasis_can_pair("0.039***", '*'));
/// assert!(!emphasis_can_pair("*8 ml", '*'));
/// assert!(emphasis_can_pair("x*y*z", '*'));
/// assert!(emphasis_can_pair("a _foo_ b", '_'));
/// assert!(!emphasis_can_pair("snake_case_name", '_'));
/// ```
pub fn emphasis_can_pair(text: &str, delimiter: char) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let mut opener_seen = false;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != delimiter {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i] == delimiter {
            i += 1;
        }
        let before = start.checked_sub(1).map(|b| chars[b]);
        let after = chars.get(i).copied();
        let space = |c: Option<char>| c.is_none_or(char::is_whitespace);
        let punct =
            |c: Option<char>| c.is_some_and(|c| is_flanking_neutral(c) && !c.is_whitespace());
        let left = !space(after) && (!punct(after) || space(before) || punct(before));
        let right = !space(before) && (!punct(before) || space(after) || punct(after));
        let (opens, closes) = if delimiter == '_' {
            (
                left && (!right || punct(before)),
                right && (!left || punct(after)),
            )
        } else {
            (left, right)
        };
        if closes && opener_seen {
            return true;
        }
        opener_seen |= opens;
    }
    false
}

/// Where in `line` (already past its indentation) the character to escape stands, when the
/// line opens a block (see [`escape_block_starts`]).
fn block_marker_at(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let first = *bytes.first()?;
    // What follows a marker for it to be one: a space, a tab, or nothing.
    let ends_marker = |i: usize| matches!(bytes.get(i), None | Some(b' ' | b'\t' | b'\r'));
    let content = line.trim_end_matches([' ', '\t', '\r']);
    let only = |c: u8, spaced: bool| {
        content
            .bytes()
            .all(|b| b == c || (spaced && (b == b' ' || b == b'\t')))
    };
    match first {
        b'#' => {
            let hashes = bytes.iter().take_while(|&&b| b == b'#').count();
            (hashes <= 6 && ends_marker(hashes)).then_some(0)
        }
        b'>' => Some(0),
        b'-' | b'+' | b'*' if ends_marker(1) => Some(0),
        // A thematic break (three or more) or a setext underline (`-`, any number).
        b'-' if only(b'-', true) => Some(0),
        b'*' | b'_'
            if only(first, true) && content.bytes().filter(|&b| b == first).count() >= 3 =>
        {
            Some(0)
        }
        b'=' if only(b'=', false) => Some(0),
        b'`' | b'~' if bytes.iter().take_while(|&&b| b == first).count() >= 3 => Some(0),
        b'0'..=b'9' => {
            let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
            (digits <= 9
                && matches!(bytes.get(digits), Some(b'.' | b')'))
                && ends_marker(digits + 1))
            .then_some(digits)
        }
        _ => None,
    }
}

fn starts_character_reference(rest: &str) -> bool {
    let Some(end) = rest.find(';') else {
        return false;
    };
    let body = &rest[..end];
    if let Some(number) = body.strip_prefix('#') {
        return match number.strip_prefix(['x', 'X']) {
            Some(hex) => (1..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()),
            None => (1..=7).contains(&number.len()) && number.chars().all(|c| c.is_ascii_digit()),
        };
    }
    body.starts_with(|c: char| c.is_ascii_alphabetic())
        && (2..=32).contains(&body.len())
        && body.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Whether every `)` closes an earlier `(` and every `(` is closed. A backslash in the URL
/// does not shield a parenthesis: [`link_destination`] writes it as a literal backslash.
fn parentheses_balance(url: &str) -> bool {
    let mut depth = 0usize;
    for c in url.chars() {
        match c {
            '(' => depth += 1,
            ')' => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            _ => {}
        }
    }
    depth == 0
}

/// Whitespace or punctuation — what a delimiter may touch on its outside when its inside is
/// punctuation. CommonMark's punctuation is the Unicode P and S categories; a character that
/// is neither alphanumeric, whitespace nor a control is one of them, save for combining
/// marks, which do not stand at a run's edge.
fn is_flanking_neutral(c: char) -> bool {
    c.is_whitespace() || !(c.is_alphanumeric() || c.is_control())
}

/// A unit is neutral when the character it writes is: an escape writes punctuation.
fn unit_is_neutral(unit: &str) -> bool {
    unit.chars().last().is_some_and(is_flanking_neutral)
}

/// The byte ranges of `text[start..end]`'s units: one character, or a backslash and the ASCII
/// punctuation it escapes.
fn units(text: &str, start: usize, end: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut chars = text[start..end].char_indices().peekable();
    std::iter::from_fn(move || {
        let (i, c) = chars.next()?;
        let mut j = i + c.len_utf8();
        if c == '\\' {
            if let Some(&(k, escaped)) = chars.peek() {
                if escaped.is_ascii_punctuation() {
                    j = k + escaped.len_utf8();
                    chars.next();
                }
            }
        }
        Some((start + i, start + j))
    })
}

#[cfg(test)]
#[path = "../tests/unit/markdown.rs"]
mod tests;
