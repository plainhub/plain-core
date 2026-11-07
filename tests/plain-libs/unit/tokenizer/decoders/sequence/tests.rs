use super::*;
use crate::tokenizer::decoders::ctc::CTC;
use crate::tokenizer::pre_tokenizers::metaspace::Metaspace;

#[test]
fn sequence_basic() {
    let decoders = vec![
        DecoderWrapper::CTC(CTC::default()),
        DecoderWrapper::Metaspace(Metaspace::default()),
    ];
    let decoder = Sequence::new(decoders);
    let tokens: Vec<String> = vec!["▁", "▁", "H", "H", "i", "i", "▁", "y", "o", "u"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let out_tokens = decoder.decode(tokens).unwrap();
    assert_eq!(out_tokens, "Hi you");
}
