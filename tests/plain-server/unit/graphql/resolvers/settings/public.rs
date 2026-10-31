use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::db::Db;
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = handler(
                &request["method"].as_str().unwrap_or_default(),
                request["params"].clone(),
            );
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn fixture(
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    stub(host.clone(), handler);
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::test_support::schema::build(host, events, prefs, db, directory),
    )
}

#[tokio::test]
async fn reads_come_from_the_rust_store() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    let host = Arc::new(Host::default());
    stub(host.clone(), |method, _| {
        panic!("a read must not reach the platform: {method}")
    });
    let (events, _) = tokio::sync::broadcast::channel(16);
    let schema =
        crate::test_support::schema::build(host, events, prefs.clone(), db, dir.path().into());

    prefs.set("device_name", "Pixel").unwrap();
    prefs.set("editor_wrap_content", json!(true)).unwrap();
    prefs.set("dark_theme", 2).unwrap();
    prefs.set("audio_play_mode", 2).unwrap();
    for key in [
        "audio_sort_by",
        "video_sort_by",
        "image_sort_by",
        "doc_sort_by",
        "file_sort_by",
        "pkg_sort_by",
    ] {
        prefs.set(key, 2).unwrap();
    }

    let response = schema.execute(r#"query { prefs }"#).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        prefs.get::<String>("device_name").unwrap().unwrap(),
        "Pixel"
    );
    assert_eq!(data["prefs"]["editor_wrap_content"], json!(true));
    assert_eq!(data["prefs"]["device_name"], "Pixel");
    assert_eq!(data["prefs"]["dark_theme"], "OFF");
    assert_eq!(data["prefs"]["audio_play_mode"], "SHUFFLE");
    for key in [
        "audio_sort_by",
        "video_sort_by",
        "image_sort_by",
        "doc_sort_by",
        "file_sort_by",
        "pkg_sort_by",
    ] {
        assert_eq!(data["prefs"][key], "SIZE_ASC");
    }
    assert_eq!(prefs.get::<u64>("dark_theme").unwrap(), Some(2));
    prefs.set("http_port", 8080).unwrap();
    prefs.set("master_secret", "not public").unwrap();
    let response = schema.execute("{prefs}").await;
    let data = response.data.into_json().unwrap();
    assert!(data["prefs"].get("http_port").is_none());
    assert!(data["prefs"].get("master_secret").is_none());
    let response = schema
        .execute(
            async_graphql::Request::new("{prefs}")
                .data(crate::graphql::executor::RequestOrigin::Local),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["prefs"]["http_port"], 8080);
    assert_eq!(data["prefs"]["dark_theme"], "OFF");
    assert_eq!(data["prefs"]["audio_play_mode"], "SHUFFLE");
    assert!(data["prefs"].get("master_secret").is_none());

    prefs.set("dark_theme", 99).unwrap();
    let response = schema.execute("{prefs}").await;
    assert!(!response.errors.is_empty());
    assert_eq!(prefs.get::<u64>("dark_theme").unwrap(), Some(99));
}

#[tokio::test]
async fn preference_writes_and_removal_use_rust_storage_without_host_calls() {
    let (_directory, schema) = fixture(|method, _| panic!("Unexpected host call: {method}"));
    let response = schema
        .execute(r#"mutation { setPref(key:"editor_wrap_content",value:true) }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["setPref"], true);
    let response = schema.execute("{prefs}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["prefs"]["editor_wrap_content"],
        json!(true)
    );
    let response = schema
        .execute(r#"mutation { removePref(key:"editor_wrap_content") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let response = schema.execute("{prefs}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(
        response.data.into_json().unwrap()["prefs"]
            .get("editor_wrap_content")
            .is_none()
    );
}

/// Unknown keys are rejected before the store is changed.
#[tokio::test]
async fn pref_keys_outside_the_safe_set_are_refused() {
    let (_dir, schema) = fixture(|method, _| panic!("the platform must not be asked: {method}"));
    for key in ["", "has space", "a/b", "a$b", "emoji🎉"] {
        let response = schema
            .execute(&format!(
                r#"mutation {{ setPref(key:{key:?}, value:"1") }}"#
            ))
            .await;
        assert_eq!(response.errors.len(), 1, "{key}");
        assert_eq!(
            response.errors[0].message, "Preference is not accessible",
            "{key}"
        );
    }
    let response = schema
        .execute(&format!(
            r#"mutation {{ removePref(key:"{}") }}"#,
            "a".repeat(129)
        ))
        .await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "Preference is not accessible");
}

/// The cap is on bytes, not characters - a multi-byte value would slip past
/// a character count and blow the store limit instead.
#[tokio::test]
async fn an_oversized_pref_value_is_refused_by_its_byte_length() {
    let (_dir, schema) = fixture(|method, _| panic!("the platform must not be asked: {method}"));
    // 33_000 four-byte characters is ~132 KB of JSON but only 33k characters.
    let payload: String = std::iter::repeat_n(char::from_u32(0x1F600).unwrap(), 33_000).collect();
    let value = serde_json::to_string(&payload).unwrap();
    let document = format!("mutation {{ setPref(key: \"locale\", value: {value}) }}");
    let response = schema.execute(&document).await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(response.errors[0].message, "invalid_pref_value");
}

#[tokio::test]
async fn a_value_just_under_the_cap_is_accepted() {
    let (_dir, schema) = fixture(|method, _| panic!("Unexpected host call: {method}"));
    let response = schema
        .execute(r#"mutation { setPref(key:"locale", value:"0123456789") }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

#[test]
fn only_registered_keys_are_writable_for_each_origin() {
    use plain_rs::prefs::pref_keys::writable;
    assert!(writable("locale", false));
    assert!(writable("locale", true));
    assert!(!writable("http_port", false));
    assert!(writable("http_port", true));
    for key in [
        "master_secret",
        "client_id",
        "unknown",
        "",
        "admin:password",
    ] {
        assert!(!writable(key, false));
        assert!(!writable(key, true));
    }
}

#[cfg(feature = "core")]
#[tokio::test]
async fn saved_data_uses_kv_and_drafts_are_scoped_to_the_full_target() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let schema = core.native_graphql().unwrap();
    for (key, value) in [
        ("chat_input_text:peer:one", "peer draft"),
        ("chat_input_text:channel:one", "channel draft"),
        ("quick_note_draft", "note"),
    ] {
        let document = format!(r#"mutation {{ setSavedValue(key:{key:?},value:{value:?}) }}"#);
        let response = schema.execute(async_graphql::Request::new(document)).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let response = schema
            .execute(async_graphql::Request::new(format!(
                "{{savedValue(key:{key:?})}}"
            )))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["savedValue"], value);
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
            .execute(async_graphql::Request::new(format!(
                r#"mutation {{ setSavedValue(key:{key:?},value:"x") }}"#
            )))
            .await;
        assert!(!response.errors.is_empty(), "{key}");
    }
    core.stop_core().await;
}

#[tokio::test]
async fn remote_tools_receive_effective_app_home_features_without_local_settings() {
    let (_directory, schema) = fixture(|method, _| panic!("Unexpected host call: {method}"));
    let response = schema.execute("{prefs}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data["prefs"]["app_home_features"],
        "IMAGES|VIDEOS|AUDIO|DOCS|FILES|NOTES|FEEDS"
    );
    assert!(data["prefs"].get("http_port").is_none());
    assert!(data["prefs"].get("password").is_none());

    let response = schema
        .execute(
            async_graphql::Request::new(
                r#"mutation { setPref(key:"app_home_features", value:"NOTES|FILES") }"#,
            )
            .data(crate::graphql::executor::RequestOrigin::Local),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    for mutation in [
        r#"mutation { setPref(key:"app_home_features", value:"FILES") }"#,
        r#"mutation { removePref(key:"app_home_features") }"#,
    ] {
        let response = schema.execute(mutation).await;
        assert_eq!(response.errors.len(), 1);
        assert_eq!(response.errors[0].message, "Preference is not accessible");
    }
    let response = schema.execute("{prefs}").await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["prefs"]["app_home_features"],
        "NOTES|FILES"
    );
}
