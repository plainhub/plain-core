use super::*;
use crate::tokenizer::models::bpe::Vocab;

#[test]
fn test_serialization() {
    let vocab: Vocab = [
        ("<unk>".into(), 0),
        ("a".into(), 1),
        ("b".into(), 2),
        ("ab".into(), 3),
    ]
    .iter()
    .cloned()
    .collect();
    let bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![("a".to_string(), "b".to_string())])
        .unk_token("<unk>".to_string())
        .ignore_merges(true)
        .build()
        .unwrap();

    let legacy = r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2,"ab":3},"merges":["a b"]}"#;
    let legacy = serde_json::from_str(legacy).unwrap();
    assert_eq!(bpe, legacy);

    let data = serde_json::to_string(&bpe).unwrap();
    assert_eq!(
        data,
        r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2,"ab":3},"merges":[["a","b"]]}"#
    );
    let reconstructed = serde_json::from_str(&data).unwrap();
    assert_eq!(bpe, reconstructed);

    // With a space in the token
    let vocab: Vocab = [
        ("<unk>".into(), 0),
        ("a".into(), 1),
        ("b c d".into(), 2),
        ("ab c d".into(), 3),
    ]
    .iter()
    .cloned()
    .collect();
    let bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![("a".to_string(), "b c d".to_string())])
        .unk_token("<unk>".to_string())
        .ignore_merges(true)
        .build()
        .unwrap();
    let data = serde_json::to_string(&bpe).unwrap();
    assert_eq!(
        data,
        r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b c d":2,"ab c d":3},"merges":[["a","b c d"]]}"#
    );
    let reconstructed = serde_json::from_str(&data).unwrap();
    assert_eq!(bpe, reconstructed);
}

#[test]
fn test_serialization_ignore_merges() {
    let vocab: Vocab = [("<unk>".into(), 0), ("a".into(), 1), ("b".into(), 2)]
        .iter()
        .cloned()
        .collect();
    let mut bpe = BpeBuilder::default()
        .vocab_and_merges(vocab, vec![])
        .unk_token("<unk>".to_string())
        .ignore_merges(true)
        .build()
        .unwrap();

    let bpe_string = r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2},"merges":[]}"#;
    assert_eq!(serde_json::from_str::<BPE>(bpe_string).unwrap(), bpe);

    bpe.ignore_merges = false;
    let bpe_string = r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"vocab":{"<unk>":0,"a":1,"b":2},"merges":[]}"#;
    assert_eq!(serde_json::from_str::<BPE>(bpe_string).unwrap(), bpe);
}
