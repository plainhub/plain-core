use super::*;
use std::convert::TryInto;
use std::iter::FromIterator;

#[test]
fn piece_serde() {
    let seq_0 = Piece::Sequence {
        id: Sequence::A,
        type_id: 0,
    };
    let seq_0_s = r#"{"Sequence":{"id":"A","type_id":0}}"#;

    assert_eq!(crate::serde_json::to_string(&seq_0).unwrap(), seq_0_s);
    assert_eq!(crate::serde_json::from_str::<Piece>(seq_0_s).unwrap(), seq_0);

    let seq_1 = Piece::Sequence {
        id: Sequence::B,
        type_id: 1,
    };
    let seq_1_s = r#"{"Sequence":{"id":"B","type_id":1}}"#;
    assert_eq!(crate::serde_json::to_string(&seq_1).unwrap(), seq_1_s);
    assert_eq!(crate::serde_json::from_str::<Piece>(seq_1_s).unwrap(), seq_1);

    let spe = Piece::SpecialToken {
        id: "[CLS]".into(),
        type_id: 0,
    };
    let spe_s = r#"{"SpecialToken":{"id":"[CLS]","type_id":0}}"#;
    assert_eq!(crate::serde_json::to_string(&spe).unwrap(), spe_s);
    assert_eq!(crate::serde_json::from_str::<Piece>(spe_s).unwrap(), spe);
}

#[test]
fn piece() {
    assert_eq!(
        Ok(Piece::Sequence {
            id: Sequence::A,
            type_id: 0
        }),
        "$".try_into()
    );
    assert_eq!(
        Ok(Piece::Sequence {
            id: Sequence::B,
            type_id: 0
        }),
        "$B".try_into()
    );
    assert_eq!(
        Ok(Piece::Sequence {
            id: Sequence::A,
            type_id: 1
        }),
        "$1".try_into()
    );
    assert_eq!(
        Ok(Piece::Sequence {
            id: Sequence::B,
            type_id: 2
        }),
        "$B:2".try_into()
    );
    assert_eq!(
        Ok(Piece::Sequence {
            id: Sequence::A,
            type_id: 1
        }),
        "$:1".try_into()
    );
    assert!(Piece::try_from("$C:1").is_err());
    assert!(Piece::try_from("$A:").is_err());
}

#[test]
fn special_token_serde() {
    let simple = SpecialToken::from(("[CLS]", 0));
    let simple_s = r#"{"id":"[CLS]","ids":[0],"tokens":["[CLS]"]}"#;
    assert_eq!(crate::serde_json::to_string(&simple).unwrap(), simple_s);
    assert_eq!(
        crate::serde_json::from_str::<SpecialToken>(simple_s).unwrap(),
        simple
    );

    let complete = SpecialToken::new(
        "[2FR]".into(),
        vec![1, 2, 3],
        vec!["convert".into(), "to".into(), "FR".into()],
    )
    .unwrap();
    let complete_s = r#"{"id":"[2FR]","ids":[1,2,3],"tokens":["convert","to","FR"]}"#;
    assert_eq!(crate::serde_json::to_string(&complete).unwrap(), complete_s);
    assert_eq!(
        crate::serde_json::from_str::<SpecialToken>(complete_s).unwrap(),
        complete
    );

    let malformed = SpecialToken::new(
        "[2FR]".into(),
        vec![1, 2],
        vec!["convert".into(), "to".into(), "FR".into()],
    );
    assert!(malformed.is_err());
    let malformed = SpecialToken::new(
        "[2FR]".into(),
        vec![1, 2, 3],
        vec!["convert".into(), "FR".into()],
    );
    assert!(malformed.is_err());
}

#[test]
fn template_serde() {
    let template = Template(vec![
        Piece::Sequence {
            id: Sequence::A,
            type_id: 0,
        },
        Piece::SpecialToken {
            id: "[CLS]".into(),
            type_id: 0,
        },
    ]);
    let template_s =
        r#"[{"Sequence":{"id":"A","type_id":0}},{"SpecialToken":{"id":"[CLS]","type_id":0}}]"#;
    assert_eq!(crate::serde_json::to_string(&template).unwrap(), template_s);
    assert_eq!(
        crate::serde_json::from_str::<Template>(template_s).unwrap(),
        template
    );
}

#[test]
fn tokens_serde() {
    let tokens = Tokens::from(vec![("[CLS]", 1), ("[SEP]", 0)]);
    let tokens_s = r#"{"[CLS]":{"id":"[CLS]","ids":[1],"tokens":["[CLS]"]},"[SEP]":{"id":"[SEP]","ids":[0],"tokens":["[SEP]"]}}"#;
    let tokens_ser = crate::serde_json::to_string(&tokens).unwrap();
    assert_eq!(tokens_ser, tokens_s);
    assert_eq!(crate::serde_json::from_str::<Tokens>(tokens_s).unwrap(), tokens);
}

fn get_bert_template() -> TemplateProcessing {
    TemplateProcessingDeserializer {
        single: "[CLS] $0 [SEP]".try_into().unwrap(),
        pair: "[CLS]:0 $A:0 [SEP]:0 $B:1 [SEP]:1".try_into().unwrap(),
        special_tokens: vec![("[CLS]", 1), ("[SEP]", 0)].into(),
    }
    .into()
}

#[test]
fn template_processing_serde() {
    let template = tests::get_bert_template();
    let template_s = "{\
            \"type\":\"TemplateProcessing\",\
            \"single\":[\
                {\"SpecialToken\":{\"id\":\"[CLS]\",\"type_id\":0}},\
                {\"Sequence\":{\"id\":\"A\",\"type_id\":0}},\
                {\"SpecialToken\":{\"id\":\"[SEP]\",\"type_id\":0}}\
            ],\
            \"pair\":[\
                {\"SpecialToken\":{\"id\":\"[CLS]\",\"type_id\":0}},\
                {\"Sequence\":{\"id\":\"A\",\"type_id\":0}},\
                {\"SpecialToken\":{\"id\":\"[SEP]\",\"type_id\":0}},\
                {\"Sequence\":{\"id\":\"B\",\"type_id\":1}},\
                {\"SpecialToken\":{\"id\":\"[SEP]\",\"type_id\":1}}\
            ],\
            \"special_tokens\":{\
                \"[CLS]\":{\
                    \"id\":\"[CLS]\",\"ids\":[1],\"tokens\":[\"[CLS]\"]\
                },\
                \"[SEP]\":{\
                    \"id\":\"[SEP]\",\"ids\":[0],\"tokens\":[\"[SEP]\"]\
                }\
            }}";
    let template_ser = crate::serde_json::to_string(&template).unwrap();
    assert_eq!(template_ser, template_s);
    assert_eq!(
        crate::serde_json::from_str::<TemplateProcessing>(template_s).unwrap(),
        template
    );
}

#[test]
fn template_processing() {
    let processor = tests::get_bert_template();
    assert_eq!(processor.added_tokens(false), 2);
    assert_eq!(processor.added_tokens(true), 3);

    use crate::tokenizer::Token;
    let encoding = Encoding::from_tokens(
        vec![
            Token::new(12, "Hello".into(), (0, 5)),
            Token::new(14, "there".into(), (6, 11)),
        ],
        0,
    );
    let pair = Encoding::from_tokens(vec![Token::new(15, "pair".into(), (0, 4))], 0);
    let single_encoding = processor.process(encoding.clone(), None, true).unwrap();
    assert_eq!(
        single_encoding,
        Encoding::new(
            vec![1, 12, 14, 0],
            vec![0, 0, 0, 0],
            vec![
                "[CLS]".into(),
                "Hello".into(),
                "there".into(),
                "[SEP]".into()
            ],
            vec![None, None, None, None],
            vec![(0, 0), (0, 5), (6, 11), (0, 0)],
            vec![1, 0, 0, 1],
            vec![1, 1, 1, 1],
            vec![],
            HashMap::from_iter(vec![(0, 1..3)]),
        )
    );
    assert_eq!(single_encoding.token_to_sequence(2), Some(0));
    assert_eq!(single_encoding.token_to_sequence(3), None);
    let pair_encoding = processor.process(encoding, Some(pair), true).unwrap();
    assert_eq!(
        pair_encoding,
        Encoding::new(
            vec![1, 12, 14, 0, 15, 0],
            vec![0, 0, 0, 0, 1, 1],
            vec![
                "[CLS]".into(),
                "Hello".into(),
                "there".into(),
                "[SEP]".into(),
                "pair".into(),
                "[SEP]".into()
            ],
            vec![None, None, None, None, None, None],
            vec![(0, 0), (0, 5), (6, 11), (0, 0), (0, 4), (0, 0)],
            vec![1, 0, 0, 1, 0, 1],
            vec![1, 1, 1, 1, 1, 1],
            vec![],
            HashMap::from_iter(vec![(0, 1..3), (1, 4..5)]),
        )
    );
    assert_eq!(pair_encoding.token_to_sequence(2), Some(0));
    assert_eq!(pair_encoding.token_to_sequence(3), None);
    assert_eq!(pair_encoding.token_to_sequence(4), Some(1));
    assert_eq!(pair_encoding.token_to_sequence(5), None);
}

#[test]
fn template_processing_overflowing() {
    let processor = tests::get_bert_template();
    assert_eq!(processor.added_tokens(false), 2);
    assert_eq!(processor.added_tokens(true), 3);

    use crate::tokenizer::Token;
    let mut encoding = Encoding::from_tokens(
        vec![
            Token::new(12, "Hello".into(), (0, 5)),
            Token::new(14, "there".into(), (6, 11)),
        ],
        0,
    );
    let overflowing = Encoding::from_tokens(vec![Token::new(13, "you".into(), (12, 15))], 0);
    encoding.set_overflowing(vec![overflowing]);

    let mut pair = Encoding::from_tokens(
        vec![
            Token::new(15, "pair".into(), (0, 4)),
            Token::new(16, "with".into(), (5, 9)),
        ],
        0,
    );
    let pair_overflowing = Encoding::from_tokens(vec![Token::new(17, "info".into(), (10, 14))], 0);
    pair.set_overflowing(vec![pair_overflowing]);

    let single_encoding = processor.process(encoding.clone(), None, true).unwrap();
    assert_eq!(
        single_encoding,
        Encoding::new(
            vec![1, 12, 14, 0],
            vec![0, 0, 0, 0],
            vec![
                "[CLS]".into(),
                "Hello".into(),
                "there".into(),
                "[SEP]".into()
            ],
            vec![None, None, None, None],
            vec![(0, 0), (0, 5), (6, 11), (0, 0)],
            vec![1, 0, 0, 1],
            vec![1, 1, 1, 1],
            vec![Encoding::new(
                vec![1, 13, 0],
                vec![0, 0, 0],
                vec!["[CLS]".into(), "you".into(), "[SEP]".into()],
                vec![None, None, None],
                vec![(0, 0), (12, 15), (0, 0)],
                vec![1, 0, 1],
                vec![1, 1, 1],
                vec![],
                HashMap::from_iter(vec![(0, 1..2)]),
            )],
            HashMap::from_iter(vec![(0, 1..3)]),
        )
    );
    assert_eq!(single_encoding.token_to_sequence(2), Some(0));
    assert_eq!(single_encoding.token_to_sequence(3), None);
    let pair_encoding = processor.process(encoding, Some(pair), true).unwrap();
    println!("{pair_encoding:#?}");
    assert_eq!(
        pair_encoding,
        Encoding::new(
            vec![1, 12, 14, 0, 15, 16, 0],
            vec![0, 0, 0, 0, 1, 1, 1],
            vec![
                "[CLS]".into(),
                "Hello".into(),
                "there".into(),
                "[SEP]".into(),
                "pair".into(),
                "with".into(),
                "[SEP]".into()
            ],
            vec![None, None, None, None, None, None, None],
            vec![(0, 0), (0, 5), (6, 11), (0, 0), (0, 4), (5, 9), (0, 0)],
            vec![1, 0, 0, 1, 0, 0, 1],
            vec![1, 1, 1, 1, 1, 1, 1],
            vec![
                Encoding::new(
                    vec![1, 13, 0, 15, 16, 0],
                    vec![0, 0, 0, 1, 1, 1],
                    vec![
                        "[CLS]".into(),
                        "you".into(),
                        "[SEP]".into(),
                        "pair".into(),
                        "with".into(),
                        "[SEP]".into()
                    ],
                    vec![None, None, None, None, None, None],
                    vec![(0, 0), (12, 15), (0, 0), (0, 4), (5, 9), (0, 0)],
                    vec![1, 0, 1, 0, 0, 1],
                    vec![1, 1, 1, 1, 1, 1],
                    vec![Encoding::new(
                        vec![1, 13, 0, 17, 0],
                        vec![0, 0, 0, 0, 1],
                        vec![
                            "[CLS]".into(),
                            "you".into(),
                            "[SEP]".into(),
                            "info".into(),
                            "[SEP]".into()
                        ],
                        vec![None, None, None, None, None,],
                        vec![(0, 0), (12, 15), (0, 0), (10, 14), (0, 0)],
                        vec![1, 0, 1, 0, 1],
                        vec![1, 1, 1, 1, 1],
                        vec![],
                        HashMap::from_iter(vec![(0, 1..2), (1, 3..4)]),
                    ),],
                    HashMap::from_iter(vec![(1, 3..5), (0, 1..2)]),
                ),
                Encoding::new(
                    vec![1, 13, 0, 17, 0],
                    vec![0, 0, 0, 0, 1],
                    vec![
                        "[CLS]".into(),
                        "you".into(),
                        "[SEP]".into(),
                        "info".into(),
                        "[SEP]".into()
                    ],
                    vec![None, None, None, None, None,],
                    vec![(0, 0), (12, 15), (0, 0), (10, 14), (0, 0)],
                    vec![1, 0, 1, 0, 1],
                    vec![1, 1, 1, 1, 1],
                    vec![],
                    HashMap::from_iter(vec![(0, 1..2), (1, 3..4)]),
                ),
                Encoding::new(
                    vec![1, 12, 14, 0, 17, 0],
                    vec![0, 0, 0, 0, 0, 1],
                    vec![
                        "[CLS]".into(),
                        "Hello".into(),
                        "there".into(),
                        "[SEP]".into(),
                        "info".into(),
                        "[SEP]".into()
                    ],
                    vec![None, None, None, None, None, None],
                    vec![(0, 0), (0, 5), (6, 11), (0, 0), (10, 14), (0, 0)],
                    vec![1, 0, 0, 1, 0, 1],
                    vec![1, 1, 1, 1, 1, 1],
                    vec![Encoding::new(
                        vec![1, 13, 0, 17, 0],
                        vec![0, 0, 0, 0, 1],
                        vec![
                            "[CLS]".into(),
                            "you".into(),
                            "[SEP]".into(),
                            "info".into(),
                            "[SEP]".into()
                        ],
                        vec![None, None, None, None, None,],
                        vec![(0, 0), (12, 15), (0, 0), (10, 14), (0, 0)],
                        vec![1, 0, 1, 0, 1],
                        vec![1, 1, 1, 1, 1],
                        vec![],
                        HashMap::from_iter(vec![(0, 1..2), (1, 3..4)]),
                    ),],
                    HashMap::from_iter(vec![(0, 1..3), (1, 4..5)]),
                )
            ],
            HashMap::from_iter(vec![(0, 1..3), (1, 4..6)]),
        )
    );
    assert_eq!(pair_encoding.token_to_sequence(2), Some(0));
    assert_eq!(pair_encoding.token_to_sequence(3), None);
    assert_eq!(pair_encoding.token_to_sequence(4), Some(1));
    assert_eq!(pair_encoding.token_to_sequence(5), Some(1));
    assert_eq!(pair_encoding.token_to_sequence(6), None);
}
