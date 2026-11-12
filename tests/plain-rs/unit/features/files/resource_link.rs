use super::*;

#[test]
fn scopes_roundtrip_variable_identifiers_and_encode_only_binary_payload() {
    for scope in [
        Scope::Session("a".into()),
        Scope::Session("客户.id/?#%+".repeat(20)),
        Scope::Share("share.with.dots".into()),
    ] {
        let resource = Resource {
            r#type: if matches!(scope, Scope::Share(_)) {
                ResourceType::SharedFile
            } else {
                ResourceType::File
            },
            id: "旅行/照片.jpg".into(),
        };
        let envelope = seal(&scope, &[7; 32], &resource).unwrap();
        let (decoded, payload) = parse(&envelope).unwrap();
        assert_eq!(decoded, scope);
        assert!(!payload.contains(['=', '+', '/', '.']));
        assert_eq!(open(&decoded, &[7; 32], payload).unwrap(), resource);
        let bytes = plain_libs::utils::base64::base64_decode_checked(payload).unwrap();
        assert_eq!(
            bytes.len(),
            24 + plain_libs::serde_json::to_vec(&resource).unwrap().len() + 16
        );
        assert_ne!(seal(&scope, &[7; 32], &resource).unwrap(), envelope);
    }
}

#[test]
fn rejects_scope_substitution_wrong_key_legacy_and_unknown_resources() {
    let scope = Scope::Session("client.with.dots".into());
    let id = seal(&scope, &[7; 32], &Resource::reference("/file")).unwrap();
    let (_, payload) = parse(&id).unwrap();
    assert!(open(&Scope::Session("another".into()), &[7; 32], payload).is_err());
    assert!(open(&scope, &[8; 32], payload).is_err());
    assert!(decrypt(&scope, ResourcePurpose::ZipFiles, &[7; 32], payload).is_err());
    for invalid in [
        "",
        "oldBase64",
        "c..abc",
        "x.abc",
        "c.owner.abc=",
        "c.owner.a+b",
        "c.owner.a/b",
    ] {
        assert!(parse(invalid).is_err(), "{invalid}");
    }
    for plain in [
        r#"{"type":"UNKNOWN","id":"x"}"#,
        r#"{"type":"FILE","id":""}"#,
        r#"{"type":"FILE","id":"x","path":"/etc/secret"}"#,
        r#"{"type":"SHARED_FILE","id":"x"}"#,
    ] {
        let envelope = encrypt(&scope, ResourcePurpose::Fs, &[7; 32], plain.as_bytes()).unwrap();
        assert!(open(&scope, &[7; 32], parse(&envelope).unwrap().1).is_err());
    }
}

#[test]
fn noble_vector_decrypts_with_the_same_envelope_and_associated_data() {
    let vector: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(include_str!(
        "../../../../../plain-rs/testdata/crypto/fs-resource.json"
    ))
    .unwrap();
    let (scope, payload) = parse(vector["envelope"].as_str().unwrap()).unwrap();
    assert_eq!(
        scope,
        Scope::Session(vector["clientId"].as_str().unwrap().into())
    );
    let resource = open(
        &scope,
        &plain_libs::utils::base64::base64_decode(vector["key"].as_str().unwrap()),
        payload,
    )
    .unwrap();
    assert_eq!(
        resource,
        plain_libs::serde_json::from_value(vector["resource"].clone()).unwrap()
    );
}

#[test]
fn native_reference_input_requires_its_session() {
    let envelope = seal_reference(&[7; 32], r#"{"path":"fid:item","clientId":"client"}"#).unwrap();
    let (scope, payload) = parse(&envelope).unwrap();
    assert_eq!(scope, Scope::Session("client".into()));
    assert_eq!(
        open(&scope, &[7; 32], payload).unwrap(),
        Resource::reference("fid:item")
    );
    for input in [
        r#"{"path":"/document.docx"}"#,
        r#"{"clientId":"client"}"#,
        r#"{"path":"/document","clientId":false}"#,
        r#"{"path":"/document","clientId":""}"#,
    ] {
        assert!(seal_reference(&[7; 32], input).is_err());
    }
}
