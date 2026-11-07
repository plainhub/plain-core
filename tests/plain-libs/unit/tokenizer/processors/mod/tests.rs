use super::*;

#[test]
fn deserialize_bert_roberta_correctly() {
    let roberta = RobertaProcessing::default();
    let roberta_r = r#"{
            "type":"RobertaProcessing",
            "sep":["</s>",2],
            "cls":["<s>",0],
            "trim_offsets":true,
            "add_prefix_space":true
        }"#
    .replace(char::is_whitespace, "");
    assert_eq!(serde_json::to_string(&roberta).unwrap(), roberta_r);
    assert_eq!(
        serde_json::from_str::<PostProcessorWrapper>(&roberta_r).unwrap(),
        PostProcessorWrapper::Roberta(roberta)
    );

    let bert = BertProcessing::default();
    let bert_r = r#"{"type":"BertProcessing","sep":["[SEP]",102],"cls":["[CLS]",101]}"#;
    assert_eq!(serde_json::to_string(&bert).unwrap(), bert_r);
    assert_eq!(
        serde_json::from_str::<PostProcessorWrapper>(bert_r).unwrap(),
        PostProcessorWrapper::Bert(bert)
    );
}

#[test]
fn post_processor_deserialization_no_type() {
    let json = r#"{"add_prefix_space": true, "trim_offsets": false, "use_regex": false}"#;
    let reconstructed = serde_json::from_str::<PostProcessorWrapper>(json);
    match reconstructed {
        Err(err) => assert_eq!(
            err.to_string(),
            "data did not match any variant of untagged enum PostProcessorWrapper"
        ),
        _ => panic!("Expected an error here"),
    }

    let json = r#"{"sep":["[SEP]",102],"cls":["[CLS]",101]}"#;
    let reconstructed = serde_json::from_str::<PostProcessorWrapper>(json);
    assert!(matches!(
        reconstructed.unwrap(),
        PostProcessorWrapper::Bert(_)
    ));

    let json =
        r#"{"sep":["</s>",2], "cls":["<s>",0], "trim_offsets":true, "add_prefix_space":true}"#;
    let reconstructed = serde_json::from_str::<PostProcessorWrapper>(json);
    assert!(matches!(
        reconstructed.unwrap(),
        PostProcessorWrapper::Roberta(_)
    ));

    let json = r#"{"type":"RobertaProcessing", "sep":["</s>",2] }"#;
    let reconstructed = serde_json::from_str::<PostProcessorWrapper>(json);
    match reconstructed {
        Err(err) => assert_eq!(
            err.to_string(),
            "data did not match any variant of untagged enum PostProcessorWrapper"
        ),
        _ => panic!("Expected an error here"),
    }
}
