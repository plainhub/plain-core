use super::*;

#[test]
fn test_ordered_vocab_iter() {
    let vocab_r: VocabR = [
        (0, "a".into()),
        (1, "b".into()),
        (2, "c".into()),
        (3, "ab".into()),
    ]
    .iter()
    .cloned()
    .collect();
    let order_vocab_iter = OrderedVocabIter::new(&vocab_r);
    let serialized = crate::serde_json::to_string(&order_vocab_iter).unwrap();
    assert_eq!(serialized, "{\"a\":0,\"b\":1,\"c\":2,\"ab\":3}");
}

#[test]
fn test_unk_not_fused() {
    let vocab: Vocab = [("<unk>".into(), 0), ("a".into(), 1), ("b".into(), 2)]
        .iter()
        .cloned()
        .collect();
    let bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![])
        .unk_token("<unk>".to_string())
        .build()
        .unwrap();
    let tokens = bpe.tokenize("c").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, "<unk>".into(), (0, 1)),]);

    let tokens = bpe.tokenize("cc").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::new(0u32, "<unk>".into(), (0, 1)),
            Token::new(0u32, "<unk>".into(), (1, 2)),
        ]
    );

    let tokens = bpe.tokenize("accb").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::new(1u32, "a".into(), (0, 1)),
            Token::new(0u32, "<unk>".into(), (1, 2)),
            Token::new(0u32, "<unk>".into(), (2, 3)),
            Token::new(2u32, "b".into(), (3, 4)),
        ]
    );
}
#[test]
fn test_unk_get_fused() {
    let vocab: Vocab = [("<unk>".into(), 0), ("a".into(), 1), ("b".into(), 2)]
        .iter()
        .cloned()
        .collect();
    let bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![])
        .unk_token("<unk>".to_string())
        .fuse_unk(true)
        .build()
        .unwrap();
    let tokens = bpe.tokenize("c").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, "<unk>".into(), (0, 1)),]);

    let tokens = bpe.tokenize("cc").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, "<unk>".into(), (0, 2)),]);

    let tokens = bpe.tokenize("accb").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::new(1u32, "a".into(), (0, 1)),
            Token::new(0u32, "<unk>".into(), (1, 3)),
            Token::new(2u32, "b".into(), (3, 4)),
        ]
    );
}

#[test]
// Test tokenization. With dropout set to 0 tokenization is deterministic,
// so we know exactly what the result should be.
//
// To test this, we'll build a simple model to tokenize the word 'unrelated'.
fn test_tokenize_with_and_without_dropout() {
    let vocab: Vocab = [
        ("u".into(), 0),
        ("n".into(), 1),
        ("r".into(), 2),
        ("e".into(), 3),
        ("l".into(), 4),
        ("a".into(), 5),
        ("t".into(), 6),
        ("d".into(), 7),
        ("re".into(), 8),
        ("at".into(), 9),
        ("ed".into(), 10),
        ("un".into(), 11),
        ("ated".into(), 12),
        ("rel".into(), 13),
        ("related".into(), 14),
        ("unrelated".into(), 15),
    ]
    .iter()
    .cloned()
    .collect();
    let merges: Merges = vec![
        ("r".to_string(), "e".to_string()),
        ("a".to_string(), "t".to_string()),
        ("e".to_string(), "d".to_string()),
        ("u".to_string(), "n".to_string()),
        ("at".to_string(), "ed".to_string()),
        ("re".to_string(), "l".to_string()),
        ("rel".to_string(), "ated".to_string()),
        ("un".to_string(), "related".to_string()),
    ];
    let mut bpe = BPE::new(vocab, merges);

    // With no dropout:
    let tokens = bpe.tokenize("unrelated").unwrap();
    assert_eq!(tokens, vec![Token::new(15u32, "unrelated".into(), (0, 9))]);

    // With dropout = 0.0 (equivalent to dropout == none)
    bpe.dropout = Some(0.0);
    let tokens = bpe.tokenize("unrelated").unwrap();
    assert_eq!(tokens, vec![Token::new(15u32, "unrelated".into(), (0, 9))]);

    // Now set dropout to 1.0. Result should be no merges performed.
    bpe.dropout = Some(1.0);
    let tokens = bpe.tokenize("unrelated").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::new(0u32, "u".into(), (0, 1)),
            Token::new(1u32, "n".into(), (1, 2)),
            Token::new(2u32, "r".into(), (2, 3)),
            Token::new(3u32, "e".into(), (3, 4)),
            Token::new(4u32, "l".into(), (4, 5)),
            Token::new(5u32, "a".into(), (5, 6)),
            Token::new(6u32, "t".into(), (6, 7)),
            Token::new(3u32, "e".into(), (7, 8)),
            Token::new(7u32, "d".into(), (8, 9)),
        ]
    );

    // Now try with dropout between 0 and 1.
    bpe.dropout = Some(0.5);
    let tokens = bpe.tokenize("unrelated").unwrap();
    assert!(!tokens.is_empty() && tokens.len() <= 9);
}

#[test]
fn test_bpe_from_json() {
    let bpe: BPE = crate::serde_json::from_str(
        r#"{"vocab":{"a":0,"b":1,"c":2,"ab":3},"merges":[["a","b"]]}"#,
    )
    .unwrap();

    // Check merges.
    assert_eq!(bpe.merges.get(&(0, 1)).unwrap(), &(0u32, 3u32));

    // Check vocab.
    assert_eq!(bpe.vocab.get("a").unwrap(), &0u32);
    assert_eq!(bpe.vocab.get("b").unwrap(), &1u32);
    assert_eq!(bpe.vocab.get("c").unwrap(), &2u32);
    assert_eq!(bpe.vocab.get("ab").unwrap(), &3u32);
}

#[test]
// Ensure BPEBuilder with dropout = 0.0 doesn't error
fn test_bpe_with_dropout_0() {
    let bpe = BPE::builder().dropout(0.0).build().unwrap();
    assert_eq!(bpe.dropout, Some(0.0));
}

#[test]
// Ensure `BPE::from_file` works as expected.
fn test_bpe_with_continuing_subword_prefix() {
    let vocab: Vocab = vec![
        ("a".to_string(), 0),
        ("##b".to_string(), 1),
        ("##c".to_string(), 2),
        ("ab".to_string(), 3),
        ("abc".to_string(), 4),
    ]
    .into_iter()
    .collect();

    let merges = vec![
        ("a".to_string(), "##b".to_string()),
        ("ab".to_string(), "##c".to_string()),
    ];

    let bpe = BPE::builder()
        .vocab_and_merges(vocab, merges)
        .unk_token("[UNK]".to_string())
        .continuing_subword_prefix("##".to_string())
        .build()
        .unwrap();

    let res = bpe.tokenize("ab");
    assert_eq!(
        res.unwrap(),
        vec![Token {
            id: 3,
            value: "ab".to_string(),
            offsets: (0, 2)
        }]
    );
    let res = bpe.tokenize("abc");
    assert_eq!(
        res.unwrap(),
        vec![Token {
            id: 4,
            value: "abc".to_string(),
            offsets: (0, 3)
        }]
    );
}

#[test]
fn test_bpe_from_json_rejects_unknown_merge_token() {
    let error = crate::serde_json::from_str::<BPE>(
        r#"{"vocab":{"a":0,"b":1,"c":2,"ab":3},"merges":[["a","b"],["a","d"]]}"#,
    )
    .unwrap_err();
    assert!(error.to_string().contains("d"));
}

#[test]
fn test_bpe_from_json_rejects_malformed_merge() {
    assert!(
        crate::serde_json::from_str::<BPE>(
            r#"{"vocab":{"a":0,"b":1,"c":2,"ab":3},"merges":[["a","b"],["c"]]}"#,
        )
        .is_err()
    );
}

#[test]
fn test_bpe_byte_fallback() {
    // 0x61 == 'a' in bytes
    let vocab: Vocab = [("<unk>".into(), 0), ("<0x61>".into(), 1)]
        .iter()
        .cloned()
        .collect();
    let bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![])
        .unk_token("<unk>".to_string())
        .byte_fallback(true)
        .build()
        .unwrap();
    let tokens = bpe.tokenize("c").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, "<unk>".into(), (0, 1)),]);

    let tokens = bpe.tokenize("a").unwrap();
    assert_eq!(tokens, vec![Token::new(1u32, "<0x61>".into(), (0, 1)),]);
}

#[test]
fn test_bpe_byte_fallback_newline() {
    // 0x0A == '\n' in bytes
    let vocab: Vocab = [("<unk>".into(), 0), ("<0x0A>".into(), 1)]
        .iter()
        .cloned()
        .collect();
    let bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![])
        .unk_token("<unk>".to_string())
        .byte_fallback(true)
        .build()
        .unwrap();
    let tokens = bpe.tokenize("\n").unwrap();
    assert_eq!(tokens, vec![Token::new(1u32, "<0x0A>".into(), (0, 1)),]);
}

#[test]
fn test_ignore_merges() {
    // 0x0A == '\n' in bytes
    let vocab: Vocab = [
        (".:.:".into(), 0),
        ("Ġbelirtilen".into(), 1),
        (".".into(), 2),
        (":".into(), 3),
        ("bel".into(), 4),
        ("irtilen".into(), 5),
        ("Ġ".into(), 6),
        (".:".into(), 7),
        ("belirtilen".into(), 8),
        (".:.".into(), 9),
        ("be".into(), 10),
        ("l".into(), 11),
        ("ir".into(), 12),
        ("ti".into(), 13),
        ("en".into(), 14),
        ("irtil".into(), 15),
        ("irti".into(), 16),
        ("i".into(), 17),
        ("r".into(), 18),
        ("t".into(), 19),
        ("b".into(), 20),
        ("e".into(), 21),
        ("n".into(), 22),
    ]
    .iter()
    .cloned()
    .collect();
    let mut bpe = BpeBuilder::default()
        .vocab_and_merges(
            vocab,
            vec![
                (".".into(), ":".into()),
                ("b".into(), "e".into()),
                ("be".into(), "l".into()),
                ("i".into(), "r".into()),
                ("t".into(), "i".into()),
                ("ir".into(), "ti".into()),
                ("e".into(), "n".into()),
                ("irti".into(), "l".into()),
            ],
        )
        .ignore_merges(true)
        .build()
        .unwrap();
    let tokens = bpe.tokenize(".:.:").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, ".:.:".into(), (0, 4))]);

    let tokens = bpe.tokenize("Ġbelirtilen").unwrap();
    assert_eq!(
        tokens,
        vec![Token::new(1u32, "Ġbelirtilen".into(), (0, 12))]
    );

    bpe.ignore_merges = false;

    let tokens = bpe.tokenize(".:.:").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::new(7u32, ".:".into(), (0, 2)),
            Token::new(7u32, ".:".into(), (2, 4))
        ]
    );

    let tokens = bpe.tokenize("Ġbelirtilen").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token {
                id: 6,
                value: "Ġ".into(),
                offsets: (0, 2)
            },
            Token {
                id: 4,
                value: "bel".into(),
                offsets: (2, 5)
            },
            Token {
                id: 15,
                value: "irtil".into(),
                offsets: (5, 10)
            },
            Token {
                id: 14,
                value: "en".into(),
                offsets: (10, 12)
            }
        ]
    )
}
