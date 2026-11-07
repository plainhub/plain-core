use crate::tokenizer::models::wordlevel::{Vocab, WordLevel, WordLevelBuilder};

#[test]
fn serde() {
    let wl = WordLevel::default();
    let wl_s = r#"{"type":"WordLevel","vocab":{},"unk_token":"<unk>"}"#;

    assert_eq!(serde_json::to_string(&wl).unwrap(), wl_s);
    assert_eq!(serde_json::from_str::<WordLevel>(wl_s).unwrap(), wl);
}

#[test]
fn incomplete_vocab() {
    let vocab: Vocab = [("<unk>".into(), 0), ("b".into(), 2)]
        .iter()
        .cloned()
        .collect();
    let wordlevel = WordLevelBuilder::default()
        .vocab(vocab)
        .unk_token("<unk>".to_string())
        .build()
        .unwrap();
    let wl_s = r#"{"type":"WordLevel","vocab":{"<unk>":0,"b":2},"unk_token":"<unk>"}"#;
    assert_eq!(serde_json::to_string(&wordlevel).unwrap(), wl_s);
    assert_eq!(serde_json::from_str::<WordLevel>(wl_s).unwrap(), wordlevel);
}

#[test]
fn deserialization_should_fail() {
    let missing_unk = r#"{"type":"WordLevel","vocab":{}}"#;
    assert!(
        serde_json::from_str::<WordLevel>(missing_unk)
            .unwrap_err()
            .to_string()
            .starts_with("missing field `unk_token`")
    );

    let wrong_type = r#"{"type":"WordPiece","vocab":{}}"#;
    assert!(
        serde_json::from_str::<WordLevel>(wrong_type)
            .unwrap_err()
            .to_string()
            .starts_with("invalid value: string \"WordPiece\", expected WordLevel")
    );
}
