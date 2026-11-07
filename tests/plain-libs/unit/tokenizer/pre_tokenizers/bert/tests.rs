use super::*;
use crate::tokenizer::{NormalizedString, OffsetReferential, OffsetType};

#[test]
fn basic() {
    let pretok = BertPreTokenizer;
    let mut pretokenized: PreTokenizedString = "Hey friend!     How are you?!?".into();
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hey", (0, 3)),
            ("friend", (4, 10)),
            ("!", (10, 11)),
            ("How", (16, 19)),
            ("are", (20, 23)),
            ("you", (24, 27)),
            ("?", (27, 28)),
            ("!", (28, 29)),
            ("?", (29, 30)),
        ]
    );
}

#[test]
fn chinese_chars() {
    let mut n = NormalizedString::from("野口里佳 Noguchi Rika");
    n.transform(
        n.get().to_owned().chars().flat_map(|c| {
            if (c as usize) > 0x4E00 {
                vec![(' ', 0), (c, 1), (' ', 1)]
            } else {
                vec![(c, 0)]
            }
        }),
        0,
    );
    let mut pretokenized = n.into();
    let pretok = BertPreTokenizer;
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("野", (0, 3)),
            ("口", (3, 6)),
            ("里", (6, 9)),
            ("佳", (9, 12)),
            ("Noguchi", (13, 20)),
            ("Rika", (21, 25))
        ]
    );
}
