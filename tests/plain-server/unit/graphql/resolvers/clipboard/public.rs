use super::*;
use crate::test_support::schema::SchemaFixture as PublicSchema;
use plain_rs::db::Db;
use serde_json::{Value, json};

fn fixture(permissions: Value) -> (tempfile::TempDir, PublicSchema, Arc<Db>) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", permissions).unwrap();
    let db = Arc::new(Db::open(&dir.path().join("data.db")).unwrap());
    plain_rs::clipboard::record(&db, "hello", "web", "web", false).unwrap();
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
    let (_dir, schema, _db) = fixture(json!(["READ_CONTACTS"]));
    for query in [
        r#"query { clipboardItems(offset:0, limit:10, query:"") { id text } }"#,
        r#"query { clipboardItemCount(query:"") }"#,
        r#"mutation { deleteClipboardItems(query:"all:true") { affectedCount } }"#,
        r#"mutation { setClipboard(text:"x") }"#,
    ] {
        let response = schema.execute(query).await;
        assert_eq!(
            response.errors[0].message, "clipboard_sync_disabled",
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
    plain_rs::clipboard::record(&db, "unselected", "", "", false).unwrap();
    for selection in [
        "ids:missing".to_owned(),
        "ids:".to_owned(),
        format!("ids:{id},{id},missing"),
    ] {
        let query = format!(
            "mutation {{ deleteClipboardItems(query:{}) {{ affectedCount }} }}",
            serde_json::to_string(&selection).unwrap()
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

fn write_fixture() -> (tempfile::TempDir, crate::server::ContentServer) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    let token = plain_rs::utils::base64::base64_encode(&[9; 32]);
    let server = crate::server::ContentServer::start(dir.path(), &token, prefs).unwrap();
    (dir, server)
}

async fn write_request(state: &crate::server::ServerState, text: &str) -> async_graphql::Response {
    crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new("mutation($text:String!){setClipboard(text:$text)}")
            .variables(async_graphql::Variables::from_json(json!({"text":text}))),
        crate::graphql::executor::RequestContext::authenticated_remote(
            state.clone(),
            "fixture-client".into(),
        ),
    )
    .await
}

fn write_host(
    state: &crate::server::ServerState,
    result: Value,
) -> Arc<std::sync::Mutex<Vec<Option<plain_rs::db::ClipboardRow>>>> {
    let observations = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = observations.clone();
    let db = state.db.clone();
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
                    seen.lock().unwrap().push(item);
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
async fn set_clipboard_records_authenticated_source_before_platform_write_and_suppresses_echo() {
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
    for item in observations.iter() {
        let item = item
            .as_ref()
            .expect("history must precede the platform write");
        assert_eq!(item.source, "fixture-client");
        assert_eq!(item.label, "");
        assert!(!item.sensitive);
    }
    assert_eq!(
        observations[0].as_ref().unwrap().id,
        observations[1].as_ref().unwrap().id
    );
    drop(observations);
    let mut events = state.events.subscribe();
    plain_rs::services::clipboard::report(
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
        .execute(async_graphql::Request::new(
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
            Some("invalid_clipboard_text")
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
    assert!(
        seen.lock().unwrap()[0].is_some(),
        "baseline history remains recorded even when the SDK fails"
    );
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    let seen = write_host(&state, json!({"result":false}));
    let response = write_request(&state, &text).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["setClipboard"], false);
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn set_clipboard_does_not_write_to_the_platform_when_history_storage_fails() {
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
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 0);
    server.shutdown().await;
}
