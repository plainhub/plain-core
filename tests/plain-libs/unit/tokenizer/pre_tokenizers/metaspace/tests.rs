use crate::regex;

use super::*;
use crate::tokenizer::{OffsetReferential, OffsetType};

#[test]
fn serialization() {
    let metaspace = Metaspace::new('_', PrependScheme::Always, true);
    let metaspace_s =
        r#"{"type":"Metaspace","replacement":"_","prepend_scheme":"always","split":true}"#;
    assert_eq!(serde_json::to_string(&metaspace).unwrap(), metaspace_s);
    assert_eq!(
        serde_json::from_str::<Metaspace>(metaspace_s).unwrap(),
        metaspace
    );

    // Also check it can deserialize previous versions
    let metaspace_s = r#"{"type":"Metaspace","replacement":"_","add_prefix_space":false,"prepend_scheme":"always"}"#;
    assert!(serde_json::from_str::<Metaspace>(metaspace_s).is_err(),);

    let metaspace = Metaspace::new('_', PrependScheme::Always, true);
    let metaspace_s = r#"{"type":"Metaspace","str_rep":"_","replacement":"_","add_prefix_space":true,"prepend_scheme":"always"}"#;
    assert_eq!(
        serde_json::from_str::<Metaspace>(metaspace_s).unwrap(),
        metaspace
    );

    let metaspace_parsed: Metaspace =
        serde_json::from_str(r#"{"type":"Metaspace","replacement":"_","add_prefix_space":true}"#)
            .unwrap();
    assert_eq!(metaspace_parsed, metaspace);
}

#[test]
fn basic() {
    let pretok = Metaspace::new('▁', PrependScheme::Always, true);
    let mut pretokenized = PreTokenizedString::from("Hey friend!");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("▁Hey", (0, 6)), ("▁friend!", (6, 16))]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("▁Hey", (0, 3)), ("▁friend!", (3, 11))]
    );
}

#[test]
fn multiple_spaces() {
    let pretok = Metaspace::new('▁', PrependScheme::Always, true);
    let mut pretokenized = PreTokenizedString::from("Hey   friend!");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("▁Hey", (0, 6)),
            ("▁", (6, 9)),
            ("▁", (9, 12)),
            ("▁friend!", (12, 22)),
        ]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("▁Hey", (0, 3)),
            ("▁", (3, 4)),
            ("▁", (4, 5)),
            ("▁friend!", (5, 13)),
        ]
    );
}

#[test]
fn non_legacy_meta_space() {
    let mut pretok = Metaspace::new('▁', PrependScheme::Always, true);
    pretok.set_prepend_scheme(PrependScheme::Always);
    assert_eq!(pretok, Metaspace::new('▁', PrependScheme::Always, true));

    pretok.set_prepend_scheme(PrependScheme::Never);
    assert_eq!(pretok, Metaspace::new('▁', PrependScheme::Never, true));

    pretok.set_prepend_scheme(PrependScheme::First);
    assert_eq!(pretok, Metaspace::new('▁', PrependScheme::First, true));

    let pretok = Metaspace::new('▁', PrependScheme::First, false);
    let mut pretokenized = PreTokenizedString::from("Hey my friend <s>how▁are you");
    let re_ref = regex::compile(r"(<s>)").unwrap();
    pretokenized
        .split(|_, sequence| sequence.split(&re_ref, SplitDelimiterBehavior::Isolated))
        .expect("Bad split");

    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("▁Hey▁my▁friend▁", (0, 23)),
            ("<s>", (23, 26)),
            ("how▁are▁you", (26, 41))
        ]
    );
    let pretok = Metaspace::new('▁', PrependScheme::Always, true);
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("▁Hey", (0, 6)),
            ("▁my", (6, 11)),
            ("▁friend", (11, 20)),
            ("▁", (20, 23)),
            ("▁<s>", (23, 29)),
            ("▁how", (29, 35)),
            ("▁are", (35, 41)),
            ("▁you", (41, 47))
        ]
    );

    let pretok = Metaspace::new('▁', PrependScheme::First, false);
    let mut pretokenized = PreTokenizedString::from(" Hey <s>how"); // test with prefix
    pretokenized
        .split(|_, sequence| sequence.split(&re_ref, SplitDelimiterBehavior::Isolated))
        .expect("Bad split");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("▁Hey▁", (0, 9)), ("<s>", (9, 12)), ("how", (12, 15))]
    );

    let mut pretokenized = PreTokenizedString::from(" Hey <s>how <s>are <s> you"); // test with many splits
    pretokenized
        .split(|_, sequence| sequence.split(&re_ref, SplitDelimiterBehavior::Isolated))
        .expect("Bad split");
    pretok.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("▁Hey▁", (0, 9)),
            ("<s>", (9, 12)),
            ("how▁", (12, 18)),
            ("<s>", (18, 21)),
            ("are▁", (21, 27)),
            ("<s>", (27, 30)),
            ("▁you", (30, 36))
        ]
    );
}
#[test]
fn decode() {
    let decoder = Metaspace::new('▁', PrependScheme::Always, true);
    let res = decoder
        .decode_chain(vec!["▁Hey".into(), "▁friend!".into()])
        .unwrap();
    assert_eq!(res, vec!["Hey", " friend!"]);

    let decoder = Metaspace::new('▁', PrependScheme::Never, true);
    let res = decoder
        .decode_chain(vec!["▁Hey".into(), "▁friend!".into()])
        .unwrap();
    assert_eq!(res, vec![" Hey", " friend!"]);
}
