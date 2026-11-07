use super::*;

#[test]
fn decoder_serialization() {
    let oldjson = r#"{"type":"Sequence","decoders":[{"type":"ByteFallback"},{"type":"Metaspace","replacement":"▁","add_prefix_space":true,"prepend_scheme":"always"}]}"#;
    let olddecoder: DecoderWrapper = serde_json::from_str(oldjson).unwrap();
    let oldserialized = serde_json::to_string(&olddecoder).unwrap();
    let json = r#"{"type":"Sequence","decoders":[{"type":"ByteFallback"},{"type":"Metaspace","replacement":"▁","prepend_scheme":"always","split":true}]}"#;
    assert_eq!(oldserialized, json);

    let decoder: DecoderWrapper = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_string(&decoder).unwrap();
    assert_eq!(serialized, json);
}
#[test]
fn decoder_serialization_other_no_arg() {
    let json = r#"{"type":"Sequence","decoders":[{"type":"Fuse"},{"type":"Metaspace","replacement":"▁","prepend_scheme":"always","split":true}]}"#;
    let decoder: DecoderWrapper = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_string(&decoder).unwrap();
    assert_eq!(serialized, json);
}

#[test]
fn decoder_serialization_no_decode() {
    let json = r#"{"type":"Sequence","decoders":[{},{"type":"Metaspace","replacement":"▁","prepend_scheme":"always"}]}"#;
    let parse = serde_json::from_str::<DecoderWrapper>(json);
    match parse {
        Err(err) => assert_eq!(
            format!("{err}"),
            "data did not match any variant of untagged enum DecoderUntagged"
        ),
        _ => panic!("Expected error"),
    }

    let json = r#"{"replacement":"▁","prepend_scheme":"always"}"#;
    let parse = serde_json::from_str::<DecoderWrapper>(json);
    match parse {
        Err(err) => assert_eq!(
            format!("{err}"),
            "data did not match any variant of untagged enum DecoderUntagged"
        ),
        _ => panic!("Expected error"),
    }

    let json = r#"{"type":"Sequence","prepend_scheme":"always"}"#;
    let parse = serde_json::from_str::<DecoderWrapper>(json);
    match parse {
        Err(err) => assert_eq!(format!("{err}"), "missing field `decoders`"),
        _ => panic!("Expected error"),
    }
}
