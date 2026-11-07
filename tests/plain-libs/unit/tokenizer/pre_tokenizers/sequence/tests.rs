use super::*;
use crate::tokenizer::pre_tokenizers::{punctuation::Punctuation, whitespace::WhitespaceSplit};
use crate::tokenizer::{OffsetReferential, OffsetType};

#[test]
fn sequence_basic() {
    let pretokenizers = vec![
        PreTokenizerWrapper::WhitespaceSplit(WhitespaceSplit),
        PreTokenizerWrapper::Punctuation(Punctuation::default()),
    ];
    let pretok = Sequence::new(pretokenizers);
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
