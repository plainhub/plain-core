use super::*;
use async_graphql::{EmptySubscription, Schema};
type Mutations = crate::graphql::resolvers::settings::public::PrefsMutation;
fn fixture() -> (
    tempfile::TempDir,
    Schema<crate::graphql::resolvers::settings::public::PrefsQuery, Mutations, EmptySubscription>,
    tokio::sync::broadcast::Receiver<WsEvent>,
) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    plain_rs::prefs::security::bootstrap(&prefs).unwrap();
    let (events, receiver) = broadcast::channel(32);
    let schema = Schema::build(
        crate::graphql::resolvers::settings::public::PrefsQuery,
        crate::graphql::resolvers::settings::public::PrefsMutation,
        EmptySubscription,
    )
    .data(prefs)
    .data(events)
    .data(crate::graphql::executor::RequestOrigin::Local)
    .finish();
    (dir, schema, receiver)
}

#[tokio::test]
async fn adb_token_verification_uses_native_management_and_has_no_graphql_oracle() {
    use crate::graphql::executor::RequestOrigin;
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    state.prefs.set("adb_token", "fixture-token").unwrap();
    let management = core.native_management().unwrap();
    for (token, expected) in [("fixture-token", true), ("wrong", false), ("", false)] {
        assert_eq!(
            management.verify_adb_token(token.to_owned()).unwrap(),
            expected
        );
        let document = format!(
            "{{verifyAdbToken(token:{})}}",
            serde_json::to_string(token).unwrap()
        );
        for origin in [
            None,
            Some(RequestOrigin::Remote),
            Some(RequestOrigin::Local),
        ] {
            let mut request = async_graphql::Request::new(&document);
            if let Some(origin) = origin {
                request = request.data(origin);
            }
            let denied = state.public.execute(request).await;
            assert!(
                denied
                    .errors
                    .iter()
                    .any(|error| error.message.contains("Unknown field")
                        && error.message.contains("verifyAdbToken"))
            );
            assert!(!denied.data.to_string().contains("true"));
        }
    }
    core.stop_core().await;
}
#[tokio::test]
async fn native_preferences_preserve_defaults_and_use_storage_keys() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    state.prefs.set("editor_font_size", 20).unwrap();
    state.prefs.set("feed_auto_refresh_interval", 2345).unwrap();
    let expected = plain_rs::prefs::settings::values(&state.prefs).unwrap();
    let actual = core.native_management().unwrap().preferences().unwrap();
    for (key, value) in &expected {
        assert_eq!(&actual[key], value, "{key}");
    }
    assert_eq!(actual["feed_auto_refresh_interval"], 2345);
    core.stop_core().await;
}

#[tokio::test]
async fn graphql_settings_are_isolated_between_devices() {
    let (_a, first, mut events) = fixture();
    let (_b, second, _) = fixture();
    let response = first
        .execute("mutation { setPref(key:\"editor_font_size\",value:20) }")
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(events.recv().await.unwrap().event_type, "PREFS_UPDATED");
    let first = first.execute("{prefs}").await.data.into_json().unwrap();
    let second = second.execute("{prefs}").await.data.into_json().unwrap();
    assert_eq!(first["prefs"]["editor_font_size"], 20);
    assert!(second["prefs"]["editor_font_size"].is_null());
}
#[tokio::test]
async fn system_commands_validate_and_do_not_return_private_keys() {
    let (_dir, schema, _) = fixture();
    let invalid = schema
        .execute("mutation {setPref(key:\"password_type\",value:4)}")
        .await;
    assert!(!invalid.errors.is_empty());
    let updated = schema
        .execute("mutation {setPref(key:\"login_require_approval\",value:false)}")
        .await;
    assert!(updated.errors.is_empty(), "{:?}", updated.errors);
    let prefs = schema.data::<Arc<Prefs>>().unwrap();
    let snapshot = plain_rs::prefs::security::snapshot(prefs).unwrap();
    assert!(!snapshot.login_require_approval);
    assert!(!snapshot.client_id.is_empty());
    assert!(!snapshot.signature_public_key.is_empty());
    assert!(
        !schema
            .execute("{systemSettings{password}}")
            .await
            .errors
            .is_empty()
    );
}

#[tokio::test]
async fn runtime_security_preferences_validate_before_publishing() {
    let (_dir, schema, mut events) = fixture();
    for (key, value) in [("log_level", "\"debug\""), ("dev_token", "\"development\"")] {
        let document = format!("mutation {{ setPref(key: \"{key}\", value: {value}) }}");
        let response = schema.execute(document).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(events.try_recv().unwrap().event_type, "PREFS_UPDATED");
        assert!(events.try_recv().is_err());
    }
    for document in [
        "mutation { setPref(key: \"log_level\", value: \"invalid\") }",
        "mutation { setPref(key: \"dev_token\", value: 1) }",
    ] {
        assert!(!schema.execute(document).await.errors.is_empty());
        assert!(events.try_recv().is_err());
    }
}

#[tokio::test]
async fn result_only_user_patch_does_not_read_unselected_system_settings() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&directory.path().join("prefs.json")).unwrap());
    plain_rs::prefs::security::bootstrap(&prefs).unwrap();
    prefs
        .set(
            "volume_alias",
            serde_json::json!("malformed unrelated system data"),
        )
        .unwrap();
    let schema = Schema::build(
        crate::graphql::resolvers::settings::public::PrefsQuery,
        crate::graphql::resolvers::settings::public::PrefsMutation,
        EmptySubscription,
    )
    .data(prefs.clone())
    .finish();
    let response = schema
        .execute(r#"mutation{setPref(key:"editor_font_size",value:21)}"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["setPref"], true);
    assert_eq!(
        plain_rs::prefs::settings::snapshot(&prefs)
            .unwrap()
            .editor_font_size,
        21
    );
    let response = schema.execute("{systemSettings{clientId}}").await;
    assert!(
        !response.errors.is_empty(),
        "Internal settings are absent from GraphQL"
    );
}

#[tokio::test]
async fn generic_writes_and_removals_reject_unknown_and_rust_owned_keys() {
    let (_directory, schema, mut events) = fixture();
    for origin in [
        crate::graphql::executor::RequestOrigin::Local,
        crate::graphql::executor::RequestOrigin::Remote,
    ] {
        for key in [
            "main_state:peer",
            "client_id",
            "signature_key_pair",
            "../preferences",
            "key with spaces",
        ] {
            for document in [
                format!(r#"mutation {{ setPref(key:{key:?},value: "changed") }}"#),
                format!("mutation {{ removePref(key:{key:?}) }}"),
            ] {
                let response = schema
                    .execute(async_graphql::Request::new(document).data(origin))
                    .await;
                assert!(!response.errors.is_empty(), "{key}");
                assert!(events.try_recv().is_err());
            }
        }
    }
}

#[tokio::test]
async fn remote_cannot_write_remove_or_reset_local_security_settings() {
    let (_directory, schema, mut events) = fixture();
    for document in [
        r#"mutation { setPref(key:"http_port",value:8080) }"#,
        r#"mutation { removePref(key:"http_port") }"#,
        "mutation { resetPassword }",
        "mutation { resetUrlToken }",
        "mutation { resetAdbToken }",
        r#"mutation { setNotificationFilterMode(mode:"allowlist") { affectedCount } }"#,
    ] {
        let response = schema
            .execute(
                async_graphql::Request::new(document)
                    .data(crate::graphql::executor::RequestOrigin::Remote),
            )
            .await;
        assert!(!response.errors.is_empty(), "{document}");
        assert!(events.try_recv().is_err());
    }
}
