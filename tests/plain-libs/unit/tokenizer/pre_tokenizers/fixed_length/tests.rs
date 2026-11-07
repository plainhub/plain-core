use super::*;
use crate::tokenizer::{OffsetReferential, OffsetType, PreTokenizer};

#[test]
fn basic() {
    let tests = vec![
        (
            "Hello world",
            vec![("Hello", (0, 5)), (" worl", (5, 10)), ("d", (10, 11))],
        ),
        ("Short", vec![("Short", (0, 5))]),
        ("", vec![]),
    ];
    let pretok = FixedLength { length: 5 };
    for (s, res) in tests {
        let mut pretokenized = PreTokenizedString::from(s);
        pretok.pre_tokenize(&mut pretokenized).unwrap();
        assert_eq!(
            pretokenized
                .get_splits(OffsetReferential::Original, OffsetType::Byte)
                .into_iter()
                .map(|(s, o, _)| (s, o))
                .collect::<Vec<_>>(),
            res
        );
    }
}

#[test]
fn custom_length() {
    let pretok = FixedLength { length: 3 };
    let mut pretokenized = PreTokenizedString::from("Hello world");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hel", (0, 3)),
            ("lo ", (3, 6)),
            ("wor", (6, 9)),
            ("ld", (9, 11)),
        ]
    );
}

#[test]
fn utf8_characters() {
    let pretok = FixedLength { length: 3 };
    let mut pretokenized = PreTokenizedString::from("Hello 👋 world");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hel", (0, 3)),
            ("lo ", (3, 6)),
            ("👋 w", (6, 12)),
            ("orl", (12, 15)),
            ("d", (15, 16)),
        ]
    );
}
