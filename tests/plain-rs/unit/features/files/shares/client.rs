use super::*;
use plain_libs::protocols::http_client;
#[test]
fn guest_wire_and_file_ids_use_existing_xchacha_contract_and_exact_query_encoding() {
    let link = Link::new(
        "::1",
        8443,
        "share +/?#中文",
        &plain_libs::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    assert!(link.page_url.starts_with("https://[::1]:8443/s/share"));
    let first = link.request(Some("folder/quoted\" 中文%")).unwrap();
    let second = link.request(None).unwrap();
    assert_ne!(&first[..24], &second[..24]);
    let plain = String::from_utf8(
        crate::infrastructure::transport::http_request::decrypt_raw(
            &[7; 32],
            "POST",
            "/guest_graphql",
            &first,
        )
        .unwrap(),
    )
    .unwrap();
    let envelope =
        crate::infrastructure::transport::graphql_request::Envelope::parse(&plain).unwrap();
    let id = envelope.id().unwrap();
    assert!(
        id.timestamp()
            .abs_diff(crate::features::peers::pairing::now_ms())
            < 1000
    );
    let document: Value = plain_libs::serde_json::from_str(&plain).unwrap();
    assert_eq!(
        document["variables"]["relativePath"],
        "folder/quoted\" 中文%"
    );
    let url = link
        .file_url(
            &plain_libs::utils::base64::base64_encode(&[8; 32]),
            "folder/+/?#中文%",
            false,
        )
        .unwrap();
    let url = http_client::Url::parse(&url).unwrap();
    let query: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert!(!query.contains_key("sid"));
    let (scope, payload) = crate::features::files::resource_link::parse(&query["id"]).unwrap();
    assert_eq!(
        scope,
        crate::features::files::resource_link::Scope::Share(link.shared_id.clone())
    );
    let file = crate::features::files::resource_link::open(&scope, &[8; 32], payload).unwrap();
    assert_eq!(
        file,
        crate::features::files::resource_link::Resource {
            r#type: crate::features::files::resource_link::ResourceType::SharedFile,
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
                &plain_libs::utils::base64::base64_encode_url_safe(&[7; 32])
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
        &plain_libs::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    let plain=json!({"data":{"sharedInfo":{"name":"share","readOnly":true,"requiresPassword":false,"expiresAt":null,"urlToken":plain_libs::utils::base64::base64_encode(&[8;32]),"entries":[]}}}).to_string();
    let request = link.request(None).unwrap();
    let context = crate::infrastructure::transport::http_response::Context::new(&request);
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

#[test]
fn share_browsing_urls_roundtrip_virtual_names_and_require_guest_links() {
    let link = Link::new(
        "localhost",
        443,
        "share-id",
        &plain_libs::utils::base64::base64_encode_url_safe(&[7; 32]),
    )
    .unwrap();
    let path = "/folder +中文/100% #?.txt";
    let url = link.directory_url(path).unwrap();
    let (parsed, relative) = Link::parse(&url).unwrap();
    assert_eq!(relative, path.trim_start_matches('/'));
    assert_eq!(parsed.shared_id, "share-id");
    assert_eq!(parsed.token, link.token);
    for invalid in [
        link.page_url.replacen("https:", "http:", 1),
        link.page_url.replacen("localhost", "user@localhost", 1),
        format!(
            "{}?q=1#{}",
            link.page_url.split('#').next().unwrap(),
            link.token
        ),
        link.page_url.split('#').next().unwrap().to_owned(),
    ] {
        assert!(Link::parse(&invalid).is_err(), "{invalid}");
    }
}
