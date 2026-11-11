use super::*;
#[test]
fn post_processor_deserialization_no_type() {
    let json = r#"{"strip_left":false, "strip_right":true}"#;
    let reconstructed = crate::serde_json::from_str::<NormalizerWrapper>(json);
    assert!(matches!(
        reconstructed.unwrap(),
        NormalizerWrapper::StripNormalizer(_)
    ));

    let json = r#"{"trim_offsets":true, "add_prefix_space":true}"#;
    let reconstructed = crate::serde_json::from_str::<NormalizerWrapper>(json);
    match reconstructed {
        Err(err) => assert_eq!(
            err.to_string(),
            "data did not match any variant of untagged enum NormalizerUntagged"
        ),
        _ => panic!("Expected an error here"),
    }

    let json = r#"{"prepend":"a"}"#;
    let reconstructed = crate::serde_json::from_str::<NormalizerWrapper>(json);
    assert!(matches!(
        reconstructed.unwrap(),
        NormalizerWrapper::Prepend(_)
    ));
}

#[test]
fn normalizer_serialization() {
    let json = r#"{"type":"Sequence","normalizers":[]}"#;
    assert!(crate::serde_json::from_str::<NormalizerWrapper>(json).is_ok());
    let json = r#"{"type":"Sequence","normalizers":[{}]}"#;
    let parse = crate::serde_json::from_str::<NormalizerWrapper>(json);
    match parse {
        Err(err) => assert_eq!(
            format!("{err}"),
            "data did not match any variant of untagged enum NormalizerUntagged"
        ),
        _ => panic!("Expected error"),
    }

    let json = r#"{"replacement":"▁","prepend_scheme":"always"}"#;
    let parse = crate::serde_json::from_str::<NormalizerWrapper>(json);
    match parse {
        Err(err) => assert_eq!(
            format!("{err}"),
            "data did not match any variant of untagged enum NormalizerUntagged"
        ),
        _ => panic!("Expected error"),
    }

    let json = r#"{"type":"Sequence","prepend_scheme":"always"}"#;
    let parse = crate::serde_json::from_str::<NormalizerWrapper>(json);
    match parse {
        Err(err) => assert_eq!(format!("{err}"), "missing field `normalizers`"),
        _ => panic!("Expected error"),
    }
}
