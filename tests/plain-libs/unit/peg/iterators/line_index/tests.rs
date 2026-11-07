use super::*;

#[test]
fn test_line_index() {
    let text = "hello 你好 A🎈C\nworld";
    let table = [
        (00, 1, 1, 'h'),
        (01, 1, 2, 'e'),
        (02, 1, 3, 'l'),
        (03, 1, 4, 'l'),
        (04, 1, 5, 'o'),
        (05, 1, 6, ' '),
        (06, 1, 7, '你'),
        (09, 1, 8, '好'),
        (12, 1, 9, ' '),
        (13, 1, 10, 'A'),
        (14, 1, 11, '🎈'),
        (18, 1, 12, 'C'),
        (19, 1, 13, '\n'),
        (20, 2, 1, 'w'),
        (21, 2, 2, 'o'),
        (22, 2, 3, 'r'),
        (23, 2, 4, 'l'),
        (24, 2, 5, 'd'),
    ];

    let index = LineIndex::new(text);
    for &(offset, line, col, c) in table.iter() {
        let res = index.line_col(text, offset);
        assert_eq!(
            (res.0, res.1),
            (line, col),
            "Expected: ({offset}, {line}, {col}, {c:?})"
        );
    }
}
