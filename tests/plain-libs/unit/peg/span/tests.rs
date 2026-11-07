use super::*;
use alloc::borrow::ToOwned;
use alloc::vec::Vec;

#[test]
fn get() {
    let input = "abc123abc";
    let span = Span::new(input, 3, input.len()).unwrap();
    assert_eq!(span.as_str(), "123abc");
    assert_eq!(span.input, input);

    let span1 = span.get(..=2);
    assert!(span1.is_some());
    assert_eq!(span1.unwrap().input, input);
    assert_eq!(span1.unwrap().as_str(), "123");

    let span2 = span.get(..);
    assert!(span2.is_some());
    assert_eq!(span2.unwrap().input, input);
    assert_eq!(span2.unwrap().as_str(), "123abc");

    let span3 = span.get(3..);
    assert!(span3.is_some());
    assert_eq!(span3.unwrap().input, input);
    assert_eq!(span3.unwrap().as_str(), "abc");

    let span4 = span.get(0..0);
    assert!(span4.is_some());
    assert_eq!(span4.unwrap().input, input);
    assert_eq!(span4.unwrap().as_str(), "");
}

#[test]
fn get_fails() {
    let input = "abc";
    let span = Span::new(input, 0, input.len()).unwrap();

    let span1 = span.get(0..100);
    assert!(span1.is_none());

    let span2 = span.get(100..200);
    assert!(span2.is_none());
}

#[test]
fn span_comp() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 1, 7).unwrap();
    let span2 = Span::new(input, 50, 51);
    assert!(span2.is_none());
    let span3 = Span::new(input, 0, 8).unwrap();
    assert!(span != span3);
}

#[test]
fn split() {
    let input = "a";
    let start = position::Position::from_start(input);
    let mut end = start;

    assert!(end.skip(1));

    let span = start.clone().span(&end.clone());

    assert_eq!(span.split(), (start, end));
}

#[test]
fn lines_mid() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 1, 7).unwrap();
    let lines: Vec<_> = span.lines().collect();
    let lines_span: Vec<_> = span.lines_span().map(|span| span.as_str()).collect();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "abc\n".to_owned());
    assert_eq!(lines[1], "def\n".to_owned());
    assert_eq!(lines, lines_span) // Verify parity with lines_span()
}

#[test]
fn lines_eof() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 5, 11).unwrap();
    assert!(span.end_pos().at_end());
    assert_eq!(span.end(), 11);
    let lines: Vec<_> = span.lines().collect();
    let lines_span: Vec<_> = span.lines_span().map(|span| span.as_str()).collect();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "def\n".to_owned());
    assert_eq!(lines[1], "ghi".to_owned());
    assert_eq!(lines, lines_span) // Verify parity with lines_span()
}

#[test]
fn lines_ending_on_line_break() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 1, 4).unwrap();
    assert_eq!(span.as_str(), "bc\n");
    let lines: Vec<_> = span.lines().collect();
    let lines_span: Vec<_> = span.lines_span().collect();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "abc\n".to_owned());
    assert_eq!(lines_span.len(), 1);
    assert_eq!(lines_span[0], Span::new(input, 0, 4).unwrap());
}

#[test]
fn lines_of_empty_span() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 5, 5).unwrap();
    assert_eq!(span.as_str(), "");
    let lines: Vec<_> = span.lines().collect();
    let lines_span: Vec<_> = span.lines_span().collect();

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "def\n".to_owned());
    assert_eq!(lines_span.len(), 1);
    assert_eq!(lines_span[0], Span::new(input, 4, 8).unwrap());
}

#[test]
fn lines_span() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 1, 7).unwrap();
    let lines_span: Vec<_> = span.lines_span().collect();
    let lines: Vec<_> = span.lines().collect();

    assert_eq!(lines_span.len(), 2);
    assert_eq!(lines_span[0], Span::new(input, 0, 4).unwrap());
    assert_eq!(lines_span[1], Span::new(input, 4, 8).unwrap());
    assert_eq!(
        lines_span
            .iter()
            .map(|span| span.as_str())
            .collect::<Vec<_>>(),
        lines
    );
}

#[test]
fn get_input_of_span() {
    let input = "abc\ndef\nghi";
    let span = Span::new(input, 1, 7).unwrap();

    assert_eq!(span.get_input(), input);
}

#[test]
fn merge_contiguous() {
    let input = "abc\ndef\nghi";
    let span1 = Span::new(input, 1, 7).unwrap();
    let span2 = Span::new(input, 7, 11).unwrap();
    let merged = merge_spans(&span1, &span2).unwrap();

    assert_eq!(merged, Span::new(input, 1, 11).unwrap());
}

#[test]
fn merge_overlapping() {
    let input = "abc\ndef\nghi";
    let span1 = Span::new(input, 1, 7).unwrap();
    let span2 = Span::new(input, 5, 11).unwrap();
    let merged = merge_spans(&span1, &span2).unwrap();

    assert_eq!(merged, Span::new(input, 1, 11).unwrap());
}

#[test]
fn merge_non_contiguous() {
    let input = "abc\ndef\nghi";
    let span1 = Span::new(input, 1, 7).unwrap();
    let span2 = Span::new(input, 8, 11).unwrap();
    let merged = merge_spans(&span1, &span2);

    assert!(merged.is_none());
}
