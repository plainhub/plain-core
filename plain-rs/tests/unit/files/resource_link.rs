use super::*;

#[test]
fn scopes_roundtrip_variable_identifiers_and_encode_only_binary_payload() {
    for scope in [
        Scope::Global,
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
        let bytes = crate::utils::base64::base64_decode_checked(payload).unwrap();
        assert_eq!(
            bytes.len(),
            24 + serde_json::to_vec(&resource).unwrap().len() + 16
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
    assert!(open(&Scope::Global, &[7; 32], payload).is_err());
    assert!(open(&scope, &[8; 32], payload).is_err());
    assert!(decrypt(&scope, "proxyfs", &[7; 32], payload).is_err());
    for invalid in [
        "",
        "oldBase64",
        "c..abc",
        "x.abc",
        "g.abc=",
        "g.a+b",
        "g.a/b",
    ] {
        assert!(parse(invalid).is_err(), "{invalid}");
    }
    for plain in [
        r#"{"type":"UNKNOWN","id":"x"}"#,
        r#"{"type":"FILE","id":""}"#,
        r#"{"type":"FILE","id":"x","path":"/etc/secret"}"#,
        r#"{"type":"SHARED_FILE","id":"x"}"#,
    ] {
        let envelope = encrypt(&scope, "fs", &[7; 32], plain.as_bytes()).unwrap();
        assert!(open(&scope, &[7; 32], parse(&envelope).unwrap().1).is_err());
    }
}

#[test]
fn noble_vector_decrypts_with_the_same_envelope_and_associated_data() {
    let vector: serde_json::Value =
        serde_json::from_str(include_str!("../../../testdata/crypto/fs-resource.json")).unwrap();
    let (scope, payload) = parse(vector["envelope"].as_str().unwrap()).unwrap();
    assert_eq!(
        scope,
        Scope::Session(vector["clientId"].as_str().unwrap().into())
    );
    let resource = open(
        &scope,
        &crate::utils::base64::base64_decode(vector["key"].as_str().unwrap()),
        payload,
    )
    .unwrap();
    assert_eq!(
        resource,
        serde_json::from_value(vector["resource"].clone()).unwrap()
    );
}
