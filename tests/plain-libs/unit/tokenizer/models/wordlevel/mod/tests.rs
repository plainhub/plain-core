use super::*;

#[test]
fn test_tokenize_unk() {
    let vocab: Vocab = [("<unk>".into(), 0), ("a".into(), 1), ("b".into(), 2)]
        .iter()
        .cloned()
        .collect();
    let wordlevel = WordLevelBuilder::default()
        .vocab(vocab)
        .unk_token("<unk>".to_string())
        .build()
        .unwrap();
    let tokens = wordlevel.tokenize("c").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, "<unk>".into(), (0, 1)),]);

    let tokens = wordlevel.tokenize("a").unwrap();
    assert_eq!(tokens, vec![Token::new(1u32, "a".into(), (0, 1)),]);
}

#[test]
fn test_tokenize_missing_unk_token() {
    let vocab: Vocab = [("a".into(), 0), ("b".into(), 1)].iter().cloned().collect();
    let wordlevel = WordLevelBuilder::default().vocab(vocab).build().unwrap();
    let tokens = wordlevel.tokenize("a").unwrap();
    assert_eq!(tokens, vec![Token::new(0u32, "a".into(), (0, 1)),]);

    let error = wordlevel.tokenize("c").err().unwrap();
    assert!(error.is::<Error>());
}
