use super::*;
use crate::tokenizer::models::bpe::{BpeBuilder, Vocab};

#[test]
fn incomplete_ordered_vocab() {
    let vocab_r: AHashMap<u32, String> =
        AHashMap::from([(0, "Hi".to_string()), (2, "There".to_string())]);

    let ordered = OrderedVocabIter::new(&vocab_r);

    let serialized = serde_json::to_string(&ordered).unwrap();
    assert_eq!(serialized, "{\"Hi\":0,\"There\":2}");
}

#[test]
fn serialization() {
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

    let model = ModelWrapper::BPE(bpe);
    let legacy = r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2,"ab":3},"merges":["a b"]}"#;
    let legacy = serde_json::from_str(legacy).unwrap();
    assert_eq!(model, legacy);

    let data = serde_json::to_string(&model).unwrap();
    assert_eq!(
        data,
        r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2,"ab":3},"merges":[["a","b"]]}"#
    );
    let reconstructed = serde_json::from_str(&data).unwrap();
    assert_eq!(model, reconstructed);

    // Legacy check, type is not necessary.
    let legacy = r#"{"dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2,"ab":3},"merges":["a b"]}"#;
    let reconstructed = serde_json::from_str(legacy).unwrap();
    assert_eq!(model, reconstructed);

    let invalid = r#"{"type":"BPE","dropout":null,"unk_token":"<unk>","continuing_subword_prefix":null,"end_of_word_suffix":null,"fuse_unk":false,"byte_fallback":false,"ignore_merges":true,"vocab":{"<unk>":0,"a":1,"b":2,"ab":3},"merges":["a b c"]}"#;
    let reconstructed: std::result::Result<ModelWrapper, serde_json::Error> =
        serde_json::from_str(invalid);
    match reconstructed {
        Err(err) => assert_eq!(err.to_string(), "Merges text file invalid at line 1"),
        _ => panic!("Expected an error here"),
    }
}
