use super::*;
use crate::tokenizer::{OffsetReferential, OffsetType};

#[test]
fn numbers() {
    let pretok = Digits::new(false);
    let mut pretokenized = PreTokenizedString::from("Hey 123 friend!");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("Hey ", (0, 4)), ("123", (4, 7)), (" friend!", (7, 15))]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("Hey ", (0, 4)), ("123", (4, 7)), (" friend!", (7, 15))]
    );
}
#[test]
fn individual_digits() {
    let pretok = Digits::new(true);
    let mut pretokenized = PreTokenizedString::from("Hey 123 friend!");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hey ", (0, 4)),
            ("1", (4, 5)),
            ("2", (5, 6)),
            ("3", (6, 7)),
            (" friend!", (7, 15))
        ]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hey ", (0, 4)),
            ("1", (4, 5)),
            ("2", (5, 6)),
            ("3", (6, 7)),
            (" friend!", (7, 15))
        ]
    );
}
