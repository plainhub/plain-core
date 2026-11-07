use super::*;
use alloc::vec;

#[test]
fn display_parsing_error_mixed() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![1, 2, 3],
            negatives: vec![4, 5, 6],
        },
        pos,
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = unexpected 4, 5, or 6; expected 1, 2, or 3"
        ]
        .join("\n")
    );
}

#[test]
fn display_parsing_error_positives() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![1, 2],
            negatives: vec![],
        },
        pos,
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = expected 1 or 2"
        ]
        .join("\n")
    );
}

#[test]
fn display_parsing_error_negatives() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![],
            negatives: vec![4, 5, 6],
        },
        pos,
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = unexpected 4, 5, or 6"
        ]
        .join("\n")
    );
}

#[test]
fn display_parsing_error_unknown() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![],
            negatives: vec![],
        },
        pos,
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = unknown parsing error"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_pos() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::CustomError {
            message: "error: big one".to_owned(),
        },
        pos,
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = error: big one"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_span_two_lines() {
    let input = "ab\ncd\nefgh";
    let start = Position::new(input, 4).unwrap();
    let end = Position::new(input, 9).unwrap();
    let error: Error<u32> = Error::new_from_span(
        ErrorVariant::CustomError {
            message: "error: big one".to_owned(),
        },
        start.span(&end),
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "3 | efgh",
            "  |  ^^",
            "  |",
            "  = error: big one"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_span_three_lines() {
    let input = "ab\ncd\nefgh";
    let start = Position::new(input, 1).unwrap();
    let end = Position::new(input, 9).unwrap();
    let error: Error<u32> = Error::new_from_span(
        ErrorVariant::CustomError {
            message: "error: big one".to_owned(),
        },
        start.span(&end),
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 1:2",
            "  |",
            "1 | ab",
            "  | ...",
            "3 | efgh",
            "  |  ^^",
            "  |",
            "  = error: big one"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_span_two_lines_inverted_cols() {
    let input = "abcdef\ngh";
    let start = Position::new(input, 5).unwrap();
    let end = Position::new(input, 8).unwrap();
    let error: Error<u32> = Error::new_from_span(
        ErrorVariant::CustomError {
            message: "error: big one".to_owned(),
        },
        start.span(&end),
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 1:6",
            "  |",
            "1 | abcdef",
            "2 | gh",
            "  | ^----^",
            "  |",
            "  = error: big one"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_span_end_after_newline() {
    let input = "abcdef\n";
    let start = Position::new(input, 0).unwrap();
    let end = Position::new(input, 7).unwrap();
    assert!(start.at_start());
    assert!(end.at_end());

    let error: Error<u32> = Error::new_from_span(
        ErrorVariant::CustomError {
            message: "error: big one".to_owned(),
        },
        start.span(&end),
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 1:1",
            "  |",
            "1 | abcdef␊",
            "  | ^-----^",
            "  |",
            "  = error: big one"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_span_end_after_inner_newline() {
    let input = "abcdef\nghi";
    let start = Position::new(input, 0).unwrap();
    let end = Position::new(input, 7).unwrap();
    assert!(start.at_start());
    assert!(!end.at_end());

    let error: Error<u32> = Error::new_from_span(
        ErrorVariant::CustomError {
            message: "error: big one".to_owned(),
        },
        start.span(&end),
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 1:1",
            "  |",
            "1 | abcdef␊",
            "  | ^-----^",
            "  |",
            "  = error: big one"
        ]
        .join("\n")
    );
}

#[test]
fn display_custom_span_empty() {
    let input = "";
    let start = Position::new(input, 0).unwrap();
    let end = Position::new(input, 0).unwrap();
    assert!(start.at_start());
    assert!(end.at_end());

    let error: Error<u32> = Error::new_from_span(
        ErrorVariant::CustomError {
            message: "error: empty".to_owned(),
        },
        start.span(&end),
    );

    assert_eq!(
        format!("{error}"),
        [
            " --> 1:1",
            "  |",
            "1 | ",
            "  | ^",
            "  |",
            "  = error: empty"
        ]
        .join("\n")
    );
}

#[test]
fn mapped_parsing_error() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![1, 2, 3],
            negatives: vec![4, 5, 6],
        },
        pos,
    )
    .renamed_rules(|n| format!("{}", n + 1));

    assert_eq!(
        format!("{error}"),
        [
            " --> 2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = unexpected 5, 6, or 7; expected 2, 3, or 4"
        ]
        .join("\n")
    );
}

#[test]
fn error_with_path() {
    let input = "ab\ncd\nef";
    let pos = Position::new(input, 4).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![1, 2, 3],
            negatives: vec![4, 5, 6],
        },
        pos,
    )
    .with_path("file.rs");

    assert_eq!(
        format!("{error}"),
        [
            " --> file.rs:2:2",
            "  |",
            "2 | cd",
            "  |  ^---",
            "  |",
            "  = unexpected 4, 5, or 6; expected 1, 2, or 3"
        ]
        .join("\n")
    );
}

#[test]
fn underline_with_tabs() {
    let input = "a\txbc";
    let pos = Position::new(input, 2).unwrap();
    let error: Error<u32> = Error::new_from_pos(
        ErrorVariant::ParsingError {
            positives: vec![1, 2, 3],
            negatives: vec![4, 5, 6],
        },
        pos,
    )
    .with_path("file.rs");

    assert_eq!(
        format!("{error}"),
        [
            " --> file.rs:1:3",
            "  |",
            "1 | a	xbc",
            "  |  	^---",
            "  |",
            "  = unexpected 4, 5, or 6; expected 1, 2, or 3"
        ]
        .join("\n")
    );
}

#[test]
fn pos_to_lcl_conversion() {
    let input = "input";

    let pos = Position::new(input, 2).unwrap();

    assert_eq!(LineColLocation::Pos(pos.line_col()), pos.into());
}

#[test]
fn span_to_lcl_conversion() {
    let input = "input";

    let span = Span::new(input, 2, 4).unwrap();
    let (start, end) = span.split();

    assert_eq!(
        LineColLocation::Span(start.line_col(), end.line_col()),
        span.into()
    );
}
