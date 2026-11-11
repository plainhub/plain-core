use plain_libs::serde_json::Value;

#[cfg(feature = "core")]
#[tokio::test]
async fn key_values_use_kv_and_drafts_are_scoped_to_the_full_target() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let schema = core.native_graphql().unwrap();
    for (key, value) in [
        ("chat_input_text:peer:one", "peer draft"),
        ("chat_input_text:channel:one", "channel draft"),
        ("quick_note_draft", "note"),
    ] {
        let document = format!(r#"mutation {{ setKeyValue(key:{key:?},value:{value:?}) }}"#);
        let response = schema.execute(plain_graphql::Request::new(document)).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let response = schema
            .execute(plain_graphql::Request::new(format!(
                "{{keyValue(key:{key:?})}}"
            )))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["keyValue"], value);
    }
    assert!(
        core.runtime_state()
            .prefs
            .get::<Value>("quick_note_draft")
            .unwrap()
            .is_none()
    );
    for key in [
        "chat_input_text:peer:",
        "chat_input_text:unknown:one",
        "master_secret",
        "http_port",
    ] {
        let response = schema
            .execute(plain_graphql::Request::new(format!(
                r#"mutation {{ setKeyValue(key:{key:?},value:"x") }}"#
            )))
            .await;
        assert!(!response.errors.is_empty(), "{key}");
    }
    core.stop_core().await;
}

#[tokio::test]
async fn key_values_report_changes_reject_null_and_enforce_encoded_byte_limit() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let schema = core.native_graphql().unwrap();
    let response = schema
        .execute(plain_graphql::Request::new(
            r#"{keyValue(key:"quick_note_draft")}"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(response.data.into_json().unwrap()["keyValue"].is_null());
    for (value, changed) in [("draft", true), ("draft", false), ("updated", true)] {
        let response = schema
            .execute(plain_graphql::Request::new(format!(
                r#"mutation {{setKeyValue(key:"quick_note_draft",value:{value:?})}}"#
            )))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["setKeyValue"], changed);
    }
    let boundary = "a".repeat(65534);
    for (value, accepted) in [(boundary.clone(), true), ("界".repeat(22000), false)] {
        let response = schema
            .execute(plain_graphql::Request::new(format!(
                r#"mutation {{setKeyValue(key:"quick_note_draft",value:{value:?})}}"#
            )))
            .await;
        if accepted {
            assert!(response.errors.is_empty(), "{:?}", response.errors);
        } else {
            assert_eq!(response.errors[0].message, "invalid_key_value");
        }
    }
    let response = schema
        .execute(plain_graphql::Request::new(
            r#"{keyValue(key:"quick_note_draft")}"#,
        ))
        .await;
    assert_eq!(response.data.into_json().unwrap()["keyValue"], boundary);
    let response = schema
        .execute(plain_graphql::Request::new(
            r#"mutation {setKeyValue(key:"quick_note_draft",value:null)}"#,
        ))
        .await;
    assert!(!response.errors.is_empty());
    assert_eq!(
        core.runtime_state()
            .store
            .get()
            .unwrap()
            .get_json::<String>("quick_note_draft")
            .unwrap(),
        Some(boundary)
    );
    for name in ["savedValue", "setSavedValue"] {
        let operation = if name == "savedValue" {
            format!(r#"{{{name}(key:"quick_note_draft")}}"#)
        } else {
            format!(r#"mutation {{{name}(key:"quick_note_draft",value:"")}}"#)
        };
        assert!(
            !schema
                .execute(plain_graphql::Request::new(operation))
                .await
                .errors
                .is_empty()
        );
    }
    core.stop_core().await;
}
