use super::*;
use crate::tokenizer::{OffsetReferential, OffsetType, PreTokenizer};

#[test]
fn basic() {
    let tests = vec![
        (
            "Hey man!",
            vec![("Hey", (0, 3)), ("man", (4, 7)), ("!", (7, 8))],
        ),
        (
            "How are you doing?",
            vec![
                ("How", (0, 3)),
                ("are", (4, 7)),
                ("you", (8, 11)),
                ("doing", (12, 17)),
                ("?", (17, 18)),
            ],
        ),
        ("\n", vec![]),
    ];
    let pretok = Whitespace {};
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
fn whitespace_split() {
    let tests = vec![
        ("Hey man!", vec![("Hey", (0, 3)), ("man!", (4, 8))]),
        (
            "Hey, man, Good?",
            vec![("Hey,", (0, 4)), ("man,", (5, 9)), ("Good?", (10, 15))],
        ),
    ];
    let pretok = WhitespaceSplit;
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
