use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_libs::serde_json::{Value, json};
use plain_rs::infrastructure::db::Db;

fn fixture(permissions: Value) -> (plain_libs::temporary::TempDir, PublicSchema, Arc<Db>) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    plain_rs::features::clipboard::record(&db, "hello", "web", "web", false).unwrap();
    let host = Arc::new(Host::default());
    let directory = dir.path().to_path_buf();
    let (events, _) = tokio::sync::broadcast::channel(16);
    (
        dir,
        crate::test_support::schema::build(host, events, prefs, db.clone(), directory),
        db,
    )
}

#[tokio::test]
async fn clipboard_roots_refuse_to_answer_without_the_web_permission() {
    let (_dir, schema, _db) = fixture(json!(["CONTACTS"]));
    for query in [
        r#"query { clipboardItems(offset:0, limit:10, query:"") { id text } }"#,
        r#"query { clipboardItemCount(query:"") }"#,
        r#"mutation { deleteClipboardItems(query:"all:true") { affectedCount } }"#,
        r#"mutation { setClipboard(text:"x") }"#,
    ] {
        let response = schema.execute(query).await;
        assert_eq!(
            response.errors[0].message, "CLIPBOARD_SYNC_DISABLED",
            "ungated clipboard root: {query}"
        );
    }
}

#[tokio::test]
async fn clipboard_page_window_is_clamped_like_the_contract() {
    let (_dir, schema, _db) = fixture(json!(["CLIPBOARD"]));
    // limit 0 would otherwise ask the database for nothing at all; the
    // contract clamps the window to at least one row.
    let response = schema
        .execute(r#"query { clipboardItems(offset:-5, limit:0, query:"") { id text source label sensitive createdAt } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let items = response.data.into_json().unwrap()["clipboardItems"].clone();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["text"], "hello");
    assert_eq!(items[0]["source"], "web");
}

#[tokio::test]
async fn delete_clipboard_items_rejects_a_blank_query() {
    let (_dir, schema, _db) = fixture(json!(["CLIPBOARD"]));
    let response = schema
        .execute(r#"mutation { deleteClipboardItems(query:"   ") { affectedCount } }"#)
        .await;
    assert!(
        response.errors[0]
            .message
            .starts_with("query is required for bulk mutations"),
        "{:?}",
        response.errors
    );

    let response = schema
        .execute(r#"mutation { deleteClipboardItems(query:"all:true") { affectedCount } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteClipboardItems"]["affectedCount"],
        1
    );
}

#[tokio::test]
async fn clipboard_id_selection_deletes_only_selected_rows() {
    let (_dir, schema, db) = fixture(json!(["CLIPBOARD"]));
    let response = schema
        .execute(r#"{ clipboardItems(offset:0, limit:10, query:"") { id } }"#)
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let id = response.data.into_json().unwrap()["clipboardItems"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    plain_rs::features::clipboard::record(&db, "unselected", "", "", false).unwrap();
    for selection in [
        "ids:missing".to_owned(),
        "ids:".to_owned(),
        format!("ids:{id},{id},missing"),
    ] {
        let query = format!(
            "mutation {{ deleteClipboardItems(query:{}) {{ affectedCount }} }}",
            plain_libs::serde_json::to_string(&selection).unwrap()
        );
        let response = schema.execute(query).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let expected = i32::from(selection.contains(&id));
        assert_eq!(
            response.data.into_json().unwrap()["deleteClipboardItems"]["affectedCount"],
            expected
        );
        let response = schema.execute(r#"{ clipboardItemCount(query:"") }"#).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["clipboardItemCount"],
            2 - expected
        );
    }
}

fn write_fixture() -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[9; 32]);
    let server = crate::server::ContentServer::start(dir.path(), &token, prefs).unwrap();
    (dir, server)
}

async fn write_request(state: &crate::server::ServerState, text: &str) -> plain_graphql::Response {
    crate::graphql::executor::execute_graphql(
        &state.public,
        plain_graphql::Request::new("mutation($text:String!){setClipboard(text:$text)}")
            .variables(plain_graphql::Variables::from_json(json!({"text":text}))),
        crate::graphql::executor::RequestContext::authenticated_remote(
            state.clone(),
            "fixture-client".into(),
        ),
    )
    .await
}

type HostWrite = (Value, Option<plain_rs::infrastructure::db::ClipboardRow>);

fn write_host(
    state: &crate::server::ServerState,
    result: Value,
) -> Arc<std::sync::Mutex<Vec<HostWrite>>> {
    let observations = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = observations.clone();
    let db = state.db.clone();
    let state = state.clone();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let reply = match request["method"].as_str().unwrap() {
                "clipboardHistoryRefresh" => json!({"result":null}),
                "systemCapabilities" => json!({"result":["CLIPBOARD"]}),
                "systemSetClipboard" => {
                    let text = request["params"]["text"].as_str().unwrap();
                    let item = db
                        .clipboard_page("", 200, 0)
                        .unwrap()
                        .into_iter()
                        .find(|item| item.text == text);
                    seen.lock().unwrap().push((request["params"].clone(), item));
                    let sensitive = request["params"]["sensitive"].as_bool().unwrap();
                    assert!(request["params"]["label"].is_string());
                    if result.get("result").and_then(Value::as_bool) == Some(true) {
                        plain_rs::features::services::clipboard::report(
                            &state,
                            &json!({"text":text,"sensitive":sensitive}).to_string(),
                        )
                        .unwrap();
                    }
                    result.clone()
                }
                other => panic!("unexpected host call {other}"),
            };
            let mut reply = reply;
            reply["id"] = request["id"].clone();
            host.reply(generation, reply).unwrap();
        }
    });
    observations
}

#[tokio::test]
async fn set_clipboard_records_after_platform_success_and_suppresses_early_and_late_echoes() {
    let (_dir, server) = write_fixture();
    let state = server.runtime_state();
    let seen = write_host(&state, json!({"result":true}));
    let mut changes = state.events.subscribe();
    for _ in 0..2 {
        let response = write_request(&state, "received 🌍").await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["setClipboard"], true);
        assert!(changes.try_recv().is_err());
    }
    let observations = seen.lock().unwrap();
    assert_eq!(observations.len(), 2);
    assert!(
        observations[0].1.is_none(),
        "history must follow platform success"
    );
    let item = observations[1].1.as_ref().unwrap();
    assert_eq!(item.source, "fixture-client");
    assert_eq!(item.label, "");
    assert!(!item.sensitive);
    assert_eq!(state.db.clipboard_latest().unwrap().unwrap().id, item.id);
    drop(observations);
    let mut events = state.events.subscribe();
    plain_rs::features::services::clipboard::report(
        &state,
        &json!({"text":"received 🌍","sensitive":false}).to_string(),
    )
    .unwrap();
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    assert!(
        events.try_recv().is_err(),
        "platform echo must not rebroadcast remotely supplied text"
    );
    let response = server
        .graphql_executor()
        .execute(plain_graphql::Request::new(
            "mutation { setClipboard(text: \"local input\") }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(
        events.try_recv().is_err(),
        "native executor must not duplicate the service event"
    );
    let local = state.db.clipboard_page("local input", 1, 0).unwrap();
    assert_eq!(local[0].source, state.native_client_id().unwrap());
    server.shutdown().await;
}

#[tokio::test]
async fn set_clipboard_preserves_explicit_metadata_for_repeated_text_without_echoes() {
    let (_dir, server) = write_fixture();
    let state = server.runtime_state();
    let seen = write_host(&state, json!({"result":true}));
    let mut events = state.events.subscribe();
    for (label, sensitive) in [("first", false), ("private", true), ("private", true)] {
        let response = crate::graphql::executor::execute_graphql(
            &state.public,
            plain_graphql::Request::new(
                r#"mutation($label:String!,$sensitive:Boolean!){setClipboard(text:"repeat",label:$label,sensitive:$sensitive)}"#,
            )
            .variables(plain_graphql::Variables::from_json(json!({
                "label": label,
                "sensitive": sensitive
            }))),
            crate::graphql::executor::RequestContext::authenticated_remote(
                state.clone(),
                "fixture-client".into(),
            ),
        )
        .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["setClipboard"], true);
    }
    let observed = seen.lock().unwrap();
    let params: Vec<_> = observed.iter().map(|(params, _)| params.clone()).collect();
    assert_eq!(
        params,
        [
            json!({"text":"repeat","label":"first","sensitive":false}),
            json!({"text":"repeat","label":"private","sensitive":true}),
            json!({"text":"repeat","label":"private","sensitive":true}),
        ]
    );
    drop(observed);
    let mut rows = state.db.clipboard_page("repeat", 10, 0).unwrap();
    rows.sort_by(|a, b| a.label.cmp(&b.label));
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].label.as_str(), rows[0].sensitive),
        ("first", false)
    );
    assert_eq!(
        (rows[1].label.as_str(), rows[1].sensitive),
        ("private", true)
    );
    assert!(rows.iter().all(|row| row.source == "fixture-client"));
    assert!(
        events.try_recv().is_err(),
        "platform writes must not be sent back"
    );
    server.shutdown().await;
}

#[tokio::test]
async fn set_clipboard_rejects_invalid_text_without_history_or_platform_writes() {
    let (_dir, server) = write_fixture();
    let state = server.runtime_state();
    let seen = write_host(&state, json!({"result":true}));
    for text in [
        "".into(),
        " \t\n".into(),
        "a".repeat(256 * 1024 + 1),
        "🌍".repeat(128 * 1024 + 1),
    ] {
        let response = write_request(&state, &text).await;
        assert_eq!(
            response.errors.first().map(|error| error.message.as_str()),
            Some("INVALID_CLIPBOARD_TEXT")
        );
    }
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    server.shutdown().await;
}

#[tokio::test]
async fn set_clipboard_keeps_the_utf16_boundary_and_platform_failure_semantics() {
    let (_dir, server) = write_fixture();
    let state = server.runtime_state();
    let seen = write_host(&state, json!({"error":"fixture platform failure"}));
    let text = "🌍".repeat(128 * 1024);
    let response = write_request(&state, &text).await;
    assert_eq!(response.errors[0].message, "fixture platform failure");
    assert!(seen.lock().unwrap()[0].1.is_none());
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    let seen = write_host(&state, json!({"result":false}));
    let response = write_request(&state, &text).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["setClipboard"], false);
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    let seen = write_host(&state, json!({"result":true}));
    let response = write_request(&state, &text).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["setClipboard"], true);
    assert!(seen.lock().unwrap()[0].1.is_none());
    assert_eq!(state.db.clipboard_latest().unwrap().unwrap().text, text);
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn set_clipboard_reports_history_storage_failure_after_the_platform_write() {
    let (_dir, server) = write_fixture();
    let state = server.runtime_state();
    let seen = write_host(&state, json!({"result":true}));
    state.db.with_conn(|connection| connection.execute_batch(
        "CREATE TRIGGER fixture_reject_clipboard BEFORE INSERT ON clipboards BEGIN SELECT RAISE(ABORT, 'fixture storage failure'); END;"
    )).unwrap();
    let response = write_request(&state, "fixture input").await;
    assert!(
        response.errors[0]
            .message
            .contains("fixture storage failure")
    );
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(seen.lock().unwrap()[0].1.is_none());
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    server.shutdown().await;
}

#[tokio::test]
async fn concurrent_platform_writes_cannot_republish_an_early_callback() {
    let (_dir, server) = write_fixture();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let mut writes = Vec::new();
    for text in ["first concurrent write", "second concurrent write"] {
        let request_state = state.clone();
        let pending = tokio::spawn(async move { write_request(&request_state, text).await });
        let id = loop {
            let request = tokio::time::timeout(std::time::Duration::from_secs(2), requests.recv())
                .await
                .unwrap()
                .unwrap();
            if request["method"] == "systemSetClipboard" {
                assert_eq!(request["params"]["text"], text);
                break request["id"].clone();
            }
            state
                .host
                .reply(generation, json!({"id":request["id"],"result":null}))
                .unwrap();
        };
        writes.push((text, id, pending));
    }
    let mut events = state.events.subscribe();
    for (text, _, _) in &writes {
        plain_rs::features::services::clipboard::report(
            &state,
            &json!({"text":text,"sensitive":false}).to_string(),
        )
        .unwrap();
    }
    assert_eq!(
        state.db.clipboard_count_rows().unwrap(),
        0,
        "history must wait for both platform results"
    );
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CLIPBOARD_CHANGED");
    }
    for (_, id, pending) in writes {
        state
            .host
            .reply(generation, json!({"id":id,"result":true}))
            .unwrap();
        let response = pending.await.unwrap();
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["setClipboard"], true);
    }
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 2);
    server.shutdown().await;
}
