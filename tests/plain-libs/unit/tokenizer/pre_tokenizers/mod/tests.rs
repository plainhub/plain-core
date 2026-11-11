use super::metaspace::PrependScheme;
use super::*;

#[test]
fn test_deserialize() {
    let pre_tokenizer: PreTokenizerWrapper = crate::serde_json::from_str(r#"{"type":"Sequence","pretokenizers":[{"type":"WhitespaceSplit"},{"type":"Metaspace","replacement":"▁","str_rep":"▁","add_prefix_space":true}]}"#).unwrap();

    assert_eq!(
        pre_tokenizer,
        PreTokenizerWrapper::Sequence(Sequence::new(vec![
            PreTokenizerWrapper::WhitespaceSplit(WhitespaceSplit {}),
            PreTokenizerWrapper::Metaspace(Metaspace::new('▁', PrependScheme::Always, true))
        ]))
    );

    let pre_tokenizer: PreTokenizerWrapper =
        crate::serde_json::from_str(r#"{"type":"Metaspace","replacement":"▁","add_prefix_space":true}"#)
            .unwrap();

    assert_eq!(
        pre_tokenizer,
        PreTokenizerWrapper::Metaspace(Metaspace::new('▁', PrependScheme::Always, true))
    );

    let pre_tokenizer: PreTokenizerWrapper = crate::serde_json::from_str(r#"{"type":"Sequence","pretokenizers":[{"type":"WhitespaceSplit"},{"type":"Metaspace","replacement":"▁","add_prefix_space":true}]}"#).unwrap();

    assert_eq!(
        pre_tokenizer,
        PreTokenizerWrapper::Sequence(Sequence::new(vec![
            PreTokenizerWrapper::WhitespaceSplit(WhitespaceSplit {}),
            PreTokenizerWrapper::Metaspace(Metaspace::new('▁', PrependScheme::Always, true))
        ]))
    );

    let pre_tokenizer: PreTokenizerWrapper = crate::serde_json::from_str(
            r#"{"type":"Metaspace","replacement":"▁","add_prefix_space":true, "prepend_scheme":"first"}"#,
        )
        .unwrap();

    assert_eq!(
        pre_tokenizer,
        PreTokenizerWrapper::Metaspace(Metaspace::new('▁', metaspace::PrependScheme::First, true))
    );

    let pre_tokenizer: PreTokenizerWrapper = crate::serde_json::from_str(
            r#"{"type":"Metaspace","replacement":"▁","add_prefix_space":true, "prepend_scheme":"always"}"#,
        )
        .unwrap();

    assert_eq!(
        pre_tokenizer,
        PreTokenizerWrapper::Metaspace(Metaspace::new('▁', metaspace::PrependScheme::Always, true))
    );
}

#[test]
fn test_deserialize_whitespace_split() {
    let pre_tokenizer: PreTokenizerWrapper =
        crate::serde_json::from_str(r#"{"type":"WhitespaceSplit"}"#).unwrap();
    assert_eq!(
        pre_tokenizer,
        PreTokenizerWrapper::WhitespaceSplit(WhitespaceSplit {})
    );
}

#[test]
fn pre_tokenizer_deserialization_no_type() {
    let json = r#"{"replacement":"▁","add_prefix_space":true, "prepend_scheme":"always"}}"#;
    let reconstructed = crate::serde_json::from_str::<PreTokenizerWrapper>(json);
    match reconstructed {
        Err(err) => assert_eq!(
            err.to_string(),
            "data did not match any variant of untagged enum PreTokenizerUntagged"
        ),
        _ => panic!("Expected an error here"),
    }

    let json = r#"{"type":"Metaspace", "replacement":"▁" }"#;
    let reconstructed = crate::serde_json::from_str::<PreTokenizerWrapper>(json).unwrap();
    assert_eq!(
        reconstructed,
        PreTokenizerWrapper::Metaspace(Metaspace::default())
    );

    let json = r#"{"type":"Metaspace", "add_prefix_space":true }"#;
    let reconstructed = crate::serde_json::from_str::<PreTokenizerWrapper>(json);
    match reconstructed {
        Err(err) => assert_eq!(err.to_string(), "missing field `replacement`"),
        _ => panic!("Expected an error here"),
    }
    let json = r#"{"behavior":"default_split"}"#;
    let reconstructed = crate::serde_json::from_str::<PreTokenizerWrapper>(json);
    match reconstructed {
        Err(err) => assert_eq!(
            err.to_string(),
            "data did not match any variant of untagged enum PreTokenizerUntagged"
        ),
        _ => panic!("Expected an error here"),
    }
}
