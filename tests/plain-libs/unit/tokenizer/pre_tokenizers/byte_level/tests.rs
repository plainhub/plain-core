use super::*;
use crate::tokenizer::tokenizer::{
    Decoder, Encoding, OffsetReferential, OffsetType, PostProcessor, PreTokenizedString,
    PreTokenizer,
};
use std::iter::FromIterator;

#[test]
fn pre_tokenization() {
    let bytelevel = ByteLevel::default().add_prefix_space(false);
    let mut pretokenized: PreTokenizedString = "Hello my friend, how is your day going?".into();
    bytelevel.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hello", (0, 5)),
            ("Ġmy", (5, 8)),
            ("Ġfriend", (8, 15)),
            (",", (15, 16)),
            ("Ġhow", (16, 20)),
            ("Ġis", (20, 23)),
            ("Ġyour", (23, 28)),
            ("Ġday", (28, 32)),
            ("Ġgoing", (32, 38)),
            ("?", (38, 39))
        ]
    );
}

#[test]
fn pre_tokenization_no_regex() {
    let bytelevel = ByteLevel::default().use_regex(false);
    let mut pretokenized: PreTokenizedString = "Hello my friend, how is your day going?".into();
    bytelevel.pre_tokenize(&mut pretokenized).unwrap();
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("ĠHelloĠmyĠfriend,ĠhowĠisĠyourĠdayĠgoing?", (0, 39))]
    );
}

#[test]
fn decoding() {
    let bytelevel = ByteLevel::default().add_prefix_space(false);
    assert_eq!(
        bytelevel
            .decode_chain(
                vec![
                    "Hello", "Ġmy", "Ġfriend", ",", "Ġhow", "Ġis", "Ġyour", "Ġday", "Ġgoing", "?"
                ]
                .into_iter()
                .map(|s| s.into())
                .collect::<Vec<String>>()
            )
            .unwrap(),
        vec!["Hello my friend, how is your day going?"]
    );
}

#[test]
fn add_prefix_space() {
    let bytelevel = ByteLevel::default().add_prefix_space(true);
    for s in &[
        " Hello my friend, how is your day going?",
        "Hello my friend, how is your day going?",
    ] {
        let mut pretokenized = PreTokenizedString::from(*s);
        bytelevel.pre_tokenize(&mut pretokenized).unwrap();
        assert_eq!(
            pretokenized
                .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
                .into_iter()
                .map(|(s, o, _)| (s, o))
                .collect::<Vec<_>>(),
            vec![
                ("ĠHello", (0, 7)),
                ("Ġmy", (7, 11)),
                ("Ġfriend", (11, 19)),
                (",", (19, 20)),
                ("Ġhow", (20, 25)),
                ("Ġis", (25, 29)),
                ("Ġyour", (29, 35)),
                ("Ġday", (35, 40)),
                ("Ġgoing", (40, 47)),
                ("?", (47, 48))
            ]
        );
    }
}

#[test]
fn decode_works_on_separated_tokens() {
    let samples = vec![
        "A Nuskhuri abbreviation of იესუ ქრისტე ( iesu kriste ) \" Jesus Christ \"",
        "An equal number have descenders , like p or q in English \
                 : გ , დ , ე , ვ , კ , ლ , ჟ , ტ , უ , ფ , ღ , ყ , ც",
    ];

    let bytelevel = ByteLevel::default().add_prefix_space(false);
    for sample in samples {
        let mut pretokenized = PreTokenizedString::from(sample);
        bytelevel.pre_tokenize(&mut pretokenized).unwrap();
        let separated_tokens = pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .iter()
            .flat_map(|(s, _, _)| s.split("").map(|t| t.into()))
            .collect::<Vec<_>>();
        assert_eq!(
            sample,
            bytelevel.decode_chain(separated_tokens).unwrap().join("")
        );
    }
}

#[test]
fn handling_of_newlines() {
    let mut pretokenized = PreTokenizedString::from("Hello there\nHello there");
    let bytelevel = ByteLevel::default().add_prefix_space(false);
    bytelevel.pre_tokenize(&mut pretokenized).unwrap();

    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hello", (0, 5)),
            ("Ġthere", (5, 11)),
            ("Ċ", (11, 12)),
            ("Hello", (12, 17)),
            ("Ġthere", (17, 23))
        ]
    );
}

#[test]
fn handling_of_multiple_whitespaces() {
    let mut pretokenized = PreTokenizedString::from("Hello there       dear");
    let bytelevel = ByteLevel::default().add_prefix_space(false);
    bytelevel.pre_tokenize(&mut pretokenized).unwrap();

    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![
            ("Hello", (0, 5)),
            ("Ġthere", (5, 11)),
            ("ĠĠĠĠĠĠ", (11, 17)),
            ("Ġdear", (17, 22))
        ]
    );
}

#[test]
fn offsets_when_char_split_up() {
    let input = "i⭢j";
    let mut pretokenized = PreTokenizedString::from(input);
    let bytelevel = ByteLevel::default().add_prefix_space(false);
    bytelevel.pre_tokenize(&mut pretokenized).unwrap();

    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("i", (0, 1)), ("âŃ¢", (1, 4)), ("j", (4, 5))]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Normalized, OffsetType::Byte)
            .into_iter()
            .map(|(s, o, _)| (s, o))
            .collect::<Vec<_>>(),
        vec![("i", (0, 1)), ("âŃ¢", (1, 7)), ("j", (7, 8))]
    );
    assert_eq!(
        pretokenized
            .get_splits(OffsetReferential::Original, OffsetType::Byte)
            .into_iter()
            .map(|(_, o, _)| &input[o.0..o.1])
            .collect::<Vec<_>>(),
        vec!["i", "⭢", "j"]
    );
}

#[test]
fn processor_trims_offsets_pre_tokenized() {
    // If user uses `is_pretokenized=True` we might have
    // offsets that might begin at the start of the string but are
    // NOT the first token.
    let mut encoding = Encoding::new(
        vec![0; 5],
        vec![],
        vec!["Ġl".into(), "ove".into(), "Ġl".into(), "ove".into()],
        vec![],
        vec![(0, 1), (1, 4), (0, 1), (1, 4)],
        vec![],
        vec![],
        vec![],
        HashMap::new(),
    );
    process_offsets(&mut encoding, true);
    assert_eq!(
        encoding,
        Encoding::new(
            vec![0; 5],
            vec![],
            vec!["Ġl".into(), "ove".into(), "Ġl".into(), "ove".into()],
            vec![],
            vec![(0, 1), (1, 4), (0, 1), (1, 4)],
            vec![],
            vec![],
            vec![],
            HashMap::new(),
        )
    );
}

#[test]
fn processor_trims_offsets() {
    let start = Encoding::new(
        vec![0; 5],
        vec![],
        vec![
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
        ],
        vec![],
        vec![(0, 1), (0, 11), (11, 18), (18, 25), (25, 29)],
        vec![],
        vec![],
        vec![],
        HashMap::new(),
    );
    let expected = Encoding::new(
        vec![0; 5],
        vec![0; 5],
        vec![
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
        ],
        vec![],
        vec![(0, 0), (4, 9), (13, 18), (18, 23), (29, 29)],
        vec![],
        vec![],
        vec![],
        HashMap::from_iter(vec![(0, 0..5)]),
    );

    let bytelevel = ByteLevel::default().trim_offsets(true);
    assert_eq!(
        expected,
        bytelevel.process(start.clone(), None, false).unwrap()
    );

    let pair_expected = Encoding::new(
        vec![0; 10],
        vec![0, 0, 0, 0, 0, 1, 1, 1, 1, 1],
        vec![
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
            "Ġ".into(),
            "ĠĠĠĠHelloĠĠ".into(),
            "ĠĠHello".into(),
            "HelloĠĠ".into(),
            "ĠĠĠĠ".into(),
        ],
        vec![],
        vec![
            (0, 0),
            (4, 9),
            (13, 18),
            (18, 23),
            (29, 29),
            (0, 0),
            (4, 9),
            (13, 18),
            (18, 23),
            (29, 29),
        ],
        vec![],
        vec![],
        vec![],
        HashMap::from_iter(vec![(0, 0..5), (1, 5..10)]),
    );
    assert_eq!(
        pair_expected,
        bytelevel
            .process(start.clone(), Some(start), false)
            .unwrap()
    );
}

#[test]
fn decode_unknown_characters() {
    let byte_level = ByteLevel::default();
    assert_eq!(
        byte_level
            .decode_chain(vec![
                "Hello".into(),
                "Ġthere".into(),
                "Ġdear".into(),
                "Ġfriend!".into(),
                "Ġ".into(),
                "[PA D]".into()
            ])
            .unwrap(),
        vec!["Hello there dear friend! [PA D]"]
    );
}

#[test]
fn deserialization() {
    // Before use_regex
    let byte_level: ByteLevel = serde_json::from_str(
        r#"{"type": "ByteLevel", "add_prefix_space": true, "trim_offsets": false}"#,
    )
    .unwrap();
    assert!(byte_level.use_regex);

    // Loading works, new future BC test.
    let byte_level: ByteLevel = serde_json::from_str(
            r#"{"type": "ByteLevel", "add_prefix_space": true, "trim_offsets": false, "use_regex": true}"#,
        )
        .unwrap();
    assert!(byte_level.use_regex);

    let byte_level: ByteLevel = serde_json::from_str(
            r#"{"type": "ByteLevel", "add_prefix_space": true, "trim_offsets": false, "use_regex": false}"#,
        )
        .unwrap();
    assert!(!byte_level.use_regex);
}
