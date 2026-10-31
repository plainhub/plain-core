use super::*;
#[test]
fn guest_wire_and_file_ids_use_existing_xchacha_contract_and_exact_query_encoding() {
    let link = Link::new(
        "::1",
        8443,
        "share +/?#中文",
        &crate::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    assert!(link.page_url.starts_with("https://[::1]:8443/s/share"));
    let first = link.request(Some("folder/quoted\" 中文%")).unwrap();
    let second = link.request(None).unwrap();
    assert_ne!(&first[..24], &second[..24]);
    let plain = String::from_utf8(crate::xchacha_decrypt_raw(&[7; 32], &first).unwrap()).unwrap();
    let parts: Vec<_> = plain.splitn(3, '|').collect();
    assert!(
        parts[0]
            .parse::<u64>()
            .unwrap()
            .abs_diff(crate::peers::pairing::now_ms() as u64)
            < 1000
    );
    assert!(uuid::Uuid::parse_str(parts[1]).is_ok());
    let document: Value = serde_json::from_str(parts[2]).unwrap();
    assert_eq!(
        document["variables"]["relativePath"],
        "folder/quoted\" 中文%"
    );
    let url = link
        .file_url(
            &crate::utils::base64::base64_encode(&[8; 32]),
            "folder/+/?#中文%",
            false,
        )
        .unwrap();
    let url = reqwest::Url::parse(&url).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert!(!query.contains_key("sid"));
    let (scope, payload) = crate::files::resource_link::parse(&query["id"]).unwrap();
    assert_eq!(
        scope,
        crate::files::resource_link::Scope::Share(link.shared_id.clone())
    );
    let file = crate::files::resource_link::open(&scope, &[8; 32], payload).unwrap();
    assert_eq!(
        file,
        crate::files::resource_link::Resource {
            r#type: crate::files::resource_link::ResourceType::SharedFile,
            id: "folder/+/?#中文%".into()
        }
    );
    assert!(link.file_url("bad", "", false).is_err());
    for host in ["localhost/path", "evil@localhost", "a,b", "a?b", "a\\b"] {
        assert!(
            Link::new(
                host,
                443,
                "id",
                &crate::utils::base64::base64_encode_url_safe(&[7; 32])
            )
            .is_err()
        );
    }
}
#[test]
fn guest_response_never_accepts_unauthenticated_data_or_invalid_file_tokens() {
    let link = Link::new(
        "localhost",
        443,
        "id",
        &crate::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    let plain=json!({"data":{"sharedInfo":{"name":"share","readOnly":true,"requiresPassword":false,"expiresAt":null,"urlToken":crate::utils::base64::base64_encode(&[8;32]),"entries":[]}}}).to_string();
    let request = link.request(None).unwrap();
    let context = crate::transport::http_response::Context::new("/guest_graphql", &request);
    assert!(link.response(&request, plain.as_bytes()).is_err());
    assert_eq!(
        link.response(
            &request,
            &context.encrypt(&[7; 32], plain.as_bytes()).unwrap()
        )
        .unwrap()
        .name,
        "share"
    );
    assert!(
        link.response(
            &request,
            &context.encrypt(&[9; 32], plain.as_bytes()).unwrap()
        )
        .is_err()
    );
    let errors = context
        .encrypt(&[7; 32], br#"{"errors":[{"message":"expired"}]}"#)
        .unwrap();
    assert_eq!(
        link.response(&request, &errors).unwrap_err().to_string(),
        "expired"
    );
}
