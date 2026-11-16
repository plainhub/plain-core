use super::*;

#[test]
fn attachments_use_the_supplied_runtime_token_without_a_session_record() {
    let token = plain_libs::crypto::gen_token();
    let content = r#"{"type":"MESSAGE","value":{"format":"MARKDOWN","text":"**caption**","items":[{"type":"FILE","attachment":{"uri":"fid:fixture.txt"}},{"type":"IMAGE","attachment":{"uri":"fid:fixture.png"}}]}}"#;
    let wire: Value = plain_libs::serde_json::from_str(
        &peer_content(content, "706c6169-26be-890e-b404-adf1389070db", &token).unwrap(),
    )
    .unwrap();
    assert_eq!(wire["value"]["format"], "MARKDOWN");
    assert_eq!(wire["value"]["text"], "**caption**");
    let items = wire["value"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let key = crate::features::sessions::token_key(&token).unwrap();
    let rotated = crate::features::sessions::token_key(&plain_libs::crypto::gen_token()).unwrap();
    for (item, kind, uri) in [
        (&items[0], "FILE", "fid:fixture.txt"),
        (&items[1], "IMAGE", "fid:fixture.png"),
    ] {
        assert_eq!(item["type"], kind);
        let id = item["attachment"]["uri"]
            .as_str()
            .unwrap()
            .strip_prefix("fsid:")
            .unwrap();
        let (scope, payload) = resource_link::parse(id).unwrap();
        assert_eq!(
            scope,
            Scope::Session("706c6169-26be-890e-b404-adf1389070db".into())
        );
        assert_eq!(
            resource_link::open(&scope, &key, payload).unwrap(),
            Resource::reference(uri)
        );
        assert!(resource_link::open(&scope, &rotated, payload).is_err());
    }
    assert!(
        peer_content(
            content,
            "706c6169-26be-890e-b404-adf1389070db",
            "invalid-token"
        )
        .is_err()
    );
}
