use crate::tokenizer::Tokenizer;

fn tokenizer(model: &str) -> Tokenizer {
    let json = format!(
        r###"{{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{model}}}"###
    );
    Tokenizer::from_bytes(json.as_bytes()).unwrap()
}

#[test]
fn all_four_json_model_types_remain_available_without_training_or_networks() {
    let models = [
        r###"{"type":"BPE","vocab":{"<unk>":0,"a":1},"merges":[],"unk_token":"<unk>"}"###,
        r###"{"type":"WordPiece","vocab":{"<unk>":0,"a":1},"unk_token":"<unk>","continuing_subword_prefix":"##","max_input_chars_per_word":100}"###,
        r###"{"type":"WordLevel","vocab":{"<unk>":0,"a":1},"unk_token":"<unk>"}"###,
        r###"{"type":"Unigram","vocab":[["<unk>",0.0],["a",-0.1]],"unk_id":0,"byte_fallback":false}"###,
    ];
    for model in models {
        let tokenizer = tokenizer(model);
        let encoding = tokenizer.encode("a", false).unwrap();
        assert_eq!(encoding.get_ids(), [1], "{model}");
        assert_eq!(encoding.get_offsets(), [(0, 1)], "{model}");
    }
}

#[test]
fn invalid_model_json_is_rejected() {
    assert!(Tokenizer::from_bytes(b"{}").is_err());
    assert!(Tokenizer::from_bytes(b"not json").is_err());
}

#[test]
fn unknown_json_metadata_is_consumed_without_weakening_model_validation() {
    let bytes = br###"{"version":"1.0","metadata":{"producer":"local","nested":[1,true,null]},"truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":{"<unk>":0,"a":1},"unk_token":"<unk>"}}"###;
    let tokenizer = Tokenizer::from_bytes(bytes).unwrap();
    assert_eq!(tokenizer.encode("a", false).unwrap().get_ids(), [1]);
    let malformed = std::str::from_utf8(bytes)
        .unwrap()
        .replace("\"WordLevel\"", "\"UnknownModel\"");
    assert!(Tokenizer::from_bytes(malformed.as_bytes()).is_err());
}
