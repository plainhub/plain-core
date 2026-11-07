use super::*;
use std::iter::FromIterator;

#[test]
fn merge_encodings() {
    let mut a = Encoding {
        ids: vec![1],
        type_ids: vec![0],
        tokens: vec![String::from("Hello ")],
        words: vec![Some(0)],
        offsets: vec![(0, 6)],
        special_tokens_mask: vec![0],
        attention_mask: vec![1],
        ..Default::default()
    };
    let b = Encoding {
        ids: vec![2],
        type_ids: vec![1],
        tokens: vec![String::from("World!")],
        words: vec![Some(0)],
        offsets: vec![(0, 6)],
        special_tokens_mask: vec![0],
        attention_mask: vec![1],
        ..Default::default()
    };
    a.merge_with(b, true);

    assert_eq!(
        a,
        Encoding {
            ids: vec![1, 2],
            type_ids: vec![0, 1],
            tokens: vec![String::from("Hello "), String::from("World!")],
            words: vec![Some(0), Some(0)],
            offsets: vec![(0, 6), (6, 12)],
            special_tokens_mask: vec![0, 0],
            attention_mask: vec![1, 1],
            ..Default::default()
        }
    );
}

#[test]
fn truncate() {
    let mut a = Encoding {
        ids: vec![1, 2, 3],
        type_ids: vec![0, 0, 0],
        tokens: vec![
            String::from("Hello"),
            String::from("World"),
            String::from("!"),
        ],
        words: vec![Some(0), Some(1), Some(2)],
        offsets: vec![(0, 5), (6, 11), (11, 12)],
        special_tokens_mask: vec![0, 0, 0],
        attention_mask: vec![1, 1, 1],
        ..Default::default()
    };
    a.truncate(2, 0, TruncationDirection::Right);

    assert_eq!(
        a,
        Encoding {
            ids: vec![1, 2],
            type_ids: vec![0, 0],
            tokens: vec![String::from("Hello"), String::from("World")],
            words: vec![Some(0), Some(1)],
            offsets: vec![(0, 5), (6, 11)],
            special_tokens_mask: vec![0, 0],
            attention_mask: vec![1, 1],
            overflowing: vec![Encoding {
                ids: vec![3],
                type_ids: vec![0],
                tokens: vec![String::from("!")],
                words: vec![Some(2)],
                offsets: vec![(11, 12)],
                special_tokens_mask: vec![0],
                attention_mask: vec![1],
                ..Default::default()
            }],
            ..Default::default()
        }
    );
}

#[test]
fn truncate_to_empty() {
    let mut a = Encoding {
        ids: vec![1, 2, 3],
        type_ids: vec![0, 0, 0],
        tokens: vec![
            String::from("Hello"),
            String::from("World"),
            String::from("!"),
        ],
        words: vec![Some(0), Some(1), Some(2)],
        offsets: vec![(0, 5), (6, 11), (11, 12)],
        special_tokens_mask: vec![0, 0, 0],
        attention_mask: vec![1, 1, 1],
        ..Default::default()
    };
    a.truncate(0, 0, TruncationDirection::Right);

    assert_eq!(
        a,
        Encoding {
            overflowing: vec![Encoding {
                ids: vec![1, 2, 3],
                type_ids: vec![0, 0, 0],
                tokens: vec![
                    String::from("Hello"),
                    String::from("World"),
                    String::from("!"),
                ],
                words: vec![Some(0), Some(1), Some(2)],
                offsets: vec![(0, 5), (6, 11), (11, 12)],
                special_tokens_mask: vec![0, 0, 0],
                attention_mask: vec![1, 1, 1],
                overflowing: vec![],
                ..Default::default()
            }],
            ..Default::default()
        }
    );
}

#[test]
fn truncate_overflow_with_stride() {
    let mut enc = Encoding {
        ids: vec![1, 2, 3, 4, 5],
        type_ids: vec![0, 0, 0, 0, 0],
        tokens: vec![
            String::from("42"),
            String::from("is"),
            String::from("the"),
            String::from("answer"),
            String::from("!"),
        ],
        words: vec![Some(0), Some(1), Some(2), Some(3), Some(4)],
        offsets: vec![(0, 2), (2, 4), (4, 7), (7, 13), (13, 14)],
        special_tokens_mask: vec![0, 0, 0, 0, 0],
        attention_mask: vec![1, 1, 1, 1, 1],
        overflowing: vec![],
        ..Default::default()
    };
    enc.truncate(4, 2, TruncationDirection::Right);

    assert_eq!(
        enc,
        Encoding {
            ids: vec![1, 2, 3, 4],
            type_ids: vec![0, 0, 0, 0],
            tokens: vec![
                String::from("42"),
                String::from("is"),
                String::from("the"),
                String::from("answer"),
            ],
            words: vec![Some(0), Some(1), Some(2), Some(3)],
            offsets: vec![(0, 2), (2, 4), (4, 7), (7, 13)],
            special_tokens_mask: vec![0, 0, 0, 0],
            attention_mask: vec![1, 1, 1, 1],
            overflowing: vec![Encoding {
                ids: vec![3, 4, 5],
                type_ids: vec![0, 0, 0],
                tokens: vec![
                    String::from("the"),
                    String::from("answer"),
                    String::from("!"),
                ],
                words: vec![Some(2), Some(3), Some(4)],
                offsets: vec![(4, 7), (7, 13), (13, 14)],
                special_tokens_mask: vec![0, 0, 0],
                attention_mask: vec![1, 1, 1],
                overflowing: vec![],
                ..Default::default()
            }],
            ..Default::default()
        }
    );
}

#[test]
fn truncate_left() {
    let mut a = Encoding {
        ids: vec![1, 2, 3],
        type_ids: vec![0, 0, 0],
        tokens: vec![
            String::from("Hello"),
            String::from("World"),
            String::from("!"),
        ],
        words: vec![Some(0), Some(1), Some(2)],
        offsets: vec![(0, 5), (6, 11), (11, 12)],
        special_tokens_mask: vec![0, 0, 0],
        attention_mask: vec![1, 1, 1],
        ..Default::default()
    };
    a.truncate(2, 0, TruncationDirection::Left);

    assert_eq!(
        a,
        Encoding {
            ids: vec![2, 3],
            type_ids: vec![0, 0],
            tokens: vec![String::from("World"), String::from("!")],
            words: vec![Some(1), Some(2)],
            offsets: vec![(6, 11), (11, 12)],
            special_tokens_mask: vec![0, 0],
            attention_mask: vec![1, 1],
            overflowing: vec![Encoding {
                ids: vec![1],
                type_ids: vec![0],
                tokens: vec![String::from("Hello")],
                words: vec![Some(0)],
                offsets: vec![(0, 5)],
                special_tokens_mask: vec![0],
                attention_mask: vec![1],
                ..Default::default()
            }],
            ..Default::default()
        }
    );
}

#[test]
fn mappings() {
    let encoding = Encoding {
        ids: vec![0; 11], // Needed for Encoding::len
        tokens: vec![
            // First sequence:
            "He".into(),
            "llo".into(),
            "won".into(),
            "der".into(),
            "ful".into(),
            "friend".into(),
            "!".into(),
            // Second sequence:
            "How".into(),
            "are".into(),
            "you".into(),
            "?".into(),
        ],
        offsets: vec![
            // First sequence:
            (0, 2),
            (2, 5),
            (7, 10),
            (10, 13),
            (13, 16),
            (17, 23),
            (23, 24),
            // Second sequence:
            (0, 3),
            (4, 7),
            (8, 11),
            (11, 12),
        ],
        words: vec![
            // First sequence:
            Some(0),
            Some(0),
            Some(1),
            Some(1),
            Some(1),
            Some(2),
            Some(3),
            // Second sequence:
            Some(0),
            Some(1),
            Some(2),
            Some(3),
        ],
        sequence_ranges: AHashMap::from_iter(vec![(0, 0..7), (1, 7..11)]),
        ..Default::default()
    };
    assert_eq!(encoding.word_to_tokens(0, 0), Some((0, 2)));
    assert_eq!(encoding.word_to_tokens(1, 0), Some((2, 5)));
    assert_eq!(encoding.word_to_tokens(2, 0), Some((5, 6)));
    assert_eq!(encoding.word_to_tokens(3, 0), Some((6, 7)));
    assert_eq!(encoding.word_to_tokens(0, 1), Some((7, 8)));
    assert_eq!(encoding.word_to_tokens(1, 1), Some((8, 9)));
    assert_eq!(encoding.word_to_tokens(2, 1), Some((9, 10)));
    assert_eq!(encoding.word_to_tokens(3, 1), Some((10, 11)));

    assert_eq!(encoding.word_to_chars(0, 0), Some((0, 5)));
    assert_eq!(encoding.word_to_chars(1, 0), Some((7, 16)));
    assert_eq!(encoding.word_to_chars(0, 1), Some((0, 3)));
    assert_eq!(encoding.word_to_chars(1, 1), Some((4, 7)));

    assert_eq!(encoding.token_to_chars(0), Some((0, (0, 2))));
    assert_eq!(encoding.token_to_chars(1), Some((0, (2, 5))));
    assert_eq!(encoding.token_to_chars(7), Some((1, (0, 3))));
    assert_eq!(encoding.token_to_chars(9), Some((1, (8, 11))));

    assert_eq!(encoding.token_to_word(1), Some((0, 0)));
    assert_eq!(encoding.token_to_word(2), Some((0, 1)));
    assert_eq!(encoding.token_to_word(7), Some((1, 0)));
    assert_eq!(encoding.token_to_word(9), Some((1, 2)));
    assert_eq!(encoding.token_to_word(11), None);

    assert_eq!(encoding.char_to_token(3, 0), Some(1));
    assert_eq!(encoding.char_to_token(8, 0), Some(2));
    assert_eq!(encoding.char_to_token(16, 0), None);
    assert_eq!(encoding.char_to_token(23, 0), Some(6));
    assert_eq!(encoding.char_to_token(2, 1), Some(7));
    assert_eq!(encoding.char_to_token(9, 1), Some(9));

    assert_eq!(encoding.char_to_word(3, 0), Some(0));
    assert_eq!(encoding.char_to_word(8, 0), Some(1));
    assert_eq!(encoding.char_to_word(16, 0), None);
    assert_eq!(encoding.char_to_word(23, 0), Some(3));
    assert_eq!(encoding.char_to_word(2, 1), Some(0));
    assert_eq!(encoding.char_to_word(9, 1), Some(2));
}

#[test]
fn padding() {
    let mut a = Encoding {
        ids: vec![1],
        type_ids: vec![0],
        tokens: vec![String::from("Hello ")],
        words: vec![Some(0)],
        offsets: vec![(0, 6)],
        special_tokens_mask: vec![0],
        attention_mask: vec![1],
        sequence_ranges: AHashMap::from([(0, 0..1)]),
        ..Default::default()
    };
    let target_length = 2;
    let pad_id = 99;
    let pad_type_id = 0;
    let pad_token = "[PAD]";
    a.pad(
        target_length,
        pad_id,
        pad_type_id,
        pad_token,
        PaddingDirection::Left,
    );
    assert_eq!(a.sequence_ranges, AHashMap::from([(0, 1..2)]));
}
