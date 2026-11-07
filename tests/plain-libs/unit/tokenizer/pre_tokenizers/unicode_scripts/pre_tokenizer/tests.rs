use super::*;
use crate::tokenizer::OffsetReferential;
use crate::tokenizer::OffsetType;

#[test]
fn basic() {
    let pretok = UnicodeScripts {};
    let mut pretokenized = PreTokenizedString::from("どこで生れ。Yes");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("どこで生れ", (0, 15)), ("。", (15, 18)), ("Yes", (18, 21))]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("どこで生れ", (0, 15)), ("。", (15, 18)), ("Yes", (18, 21))]
    );
}

#[test]
fn spaces_are_included_in_every_script() {
    let pretok = UnicodeScripts {};
    let mut pretokenized = PreTokenizedString::from("Apples are りんご 林檎");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("Apples are ", (0, 11)), ("りんご 林檎", (11, 27))]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("Apples are ", (0, 11)), ("りんご 林檎", (11, 27))]
    );
}

#[test]
fn test_unicode_script() {
    assert_eq!(Script::Han, fixed_script('京'));
    assert_eq!(Script::Han, fixed_script('太'));
    assert_eq!(Script::Han, fixed_script('い'));
    assert_eq!(Script::Han, fixed_script('グ'));
    assert_eq!(Script::Han, fixed_script('ー'));
    assert_eq!(Script::Latin, fixed_script('a'));
    assert_eq!(Script::Latin, fixed_script('A'));
    assert_eq!(Script::Common, fixed_script('0'));
    assert_eq!(Script::Common, fixed_script('$'));
    assert_eq!(Script::Common, fixed_script('@'));
    assert_eq!(Script::Common, fixed_script('-'));
    assert_eq!(Script::Any, fixed_script(' '));
}
