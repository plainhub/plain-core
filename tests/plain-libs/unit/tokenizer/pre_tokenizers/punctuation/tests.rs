use super::*;
use crate::tokenizer::{OffsetReferential, OffsetType};

#[test]
fn punctuation_basic() {
    let pretok = Punctuation::default();
    let mut pretokenized: PreTokenizedString = "Hey friend!     How are you?!?".into();
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hey friend", (0, 10)),
            ("!", (10, 11)),
            ("     How are you", (11, 27)),
            ("?", (27, 28)),
            ("!", (28, 29)),
            ("?", (29, 30)),
        ]
    );
}

#[test]
fn deserialization() {
    let punctuation: Punctuation = serde_json::from_str(r#"{"type": "Punctuation"}"#).unwrap();
    assert_eq!(punctuation, Punctuation::default());
    assert_eq!(
        punctuation,
        Punctuation::new(SplitDelimiterBehavior::Isolated)
    );
}

#[test]
#[should_panic]
fn deserialization_erroneous() {
    let _punctuation: Punctuation = serde_json::from_str(r#"{"type": "WhitespaceSplit"}"#).unwrap();
}
