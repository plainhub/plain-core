use crate::server::ContentServer;
use serde_json::{Value, json};
use std::sync::Arc;

#[tokio::test]
async fn platform_callbacks_before_write_reply_do_not_republish_received_text() {
    let directory = tempfile::tempdir().unwrap();
    let (server, _, _) = start(&directory);
    let state = server.runtime_state().core;
    let bridge = Arc::new(server.host_events().unwrap());
    let callbacks = bridge.clone();
    let history_updates = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = history_updates.clone();
    let serving = tokio::spawn(bridge.clone().serve(move |method, params| {
        let callbacks = callbacks.clone();
        let observed = observed.clone();
        async move {
            match method.as_str() {
                "clipboardHistoryRefresh" => {
                    observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(Value::Null)
                }
                "systemCapabilities" => Ok(json!(["CLIPBOARD"])),
                "systemSetClipboard" => {
                    let report = json!({"text":params["text"],"sensitive":false}).to_string();
                    for _ in 0..2 {
                        callbacks
                            .publish_text("CLIPBOARD_REPORTED", report.clone())
                            .await?;
                    }
                    Ok(json!(true))
                }
                "systemWebLoginSnapshot"
                | "peerNotificationsChanged"
                | "peerClipboardChanged"
                | "onlineClientsChanged"
                | "httpServerStatusChanged"
                | "dlnaRendererRefresh"
                | "sharedFolderDownloadsRefresh"
                | "peerTransportsChanged"
                | "appUpdateDownloadChanged"
                | "dlnaSenderChanged"
                | "feedSyncChanged"
                | "audioPlaybackRefresh"
                | "audioLibraryRefresh"
                | "networkInterfacesChanged" => Ok(Value::Null),
                _ => Err(format!("unexpected host method {method}")),
            }
        }
    }));
    let mut events = state.events.subscribe();
    let response = server
        .graphql_executor()
        .execute(async_graphql::Request::new(
            r#"mutation { setClipboard(text: "remote callback fixture") }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["setClipboard"], true);
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    assert!(!state.clipboard_changes.can_send("remote callback fixture"));
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CLIPBOARD_CHANGED");
    }
    assert_eq!(history_updates.load(std::sync::atomic::Ordering::SeqCst), 2);
    server.shutdown().await;
    serving.await.unwrap().unwrap();
    assert!(
        bridge
            .publish_text(
                "CLIPBOARD_REPORTED",
                json!({
                    "text":"after shutdown", "sensitive":false,
                })
                .to_string()
            )
            .await
            .is_err()
    );
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
}

async fn call(port: u16, token: &str, query: &str) -> Value {
    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/graphql"))
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(json!({"query": query}).to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_slice(&response.bytes().await.unwrap()).unwrap()
}

fn start(dir: &tempfile::TempDir) -> (ContentServer, String, Arc<plain_rs::prefs::Prefs>) {
    let token = plain_rs::utils::base64::base64_encode(&[9; 32]);
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("api_permissions", json!(["CLIPBOARD"])).unwrap();
    (
        ContentServer::start(dir.path(), &token, prefs.clone()).unwrap(),
        token,
        prefs,
    )
}

#[tokio::test]
async fn native_report_http_search_delete_and_restart_share_one_clipboard() {
    let dir = tempfile::tempdir().unwrap();
    let (server, token, prefs) = start(&dir);
    let bridge = server.host_events().unwrap();
    let mut events = server.runtime_state().core.events.subscribe();
    for _ in 0..15 {
        let request = bridge.next_request().await.unwrap();
        bridge
            .reply(json!({"id":request["id"],"result":null}))
            .unwrap();
    }
    let payload = json!({"text": "100%_ 🌍", "sensitive": true}).to_string();
    bridge
        .publish_text("CLIPBOARD_REPORTED", payload.clone())
        .await
        .unwrap();
    let callback = bridge.next_request().await.unwrap();
    assert_eq!(callback["method"], "clipboardHistoryRefresh");
    bridge
        .reply(json!({"id":callback["id"],"result":null}))
        .unwrap();
    assert!(events.try_recv().is_err());
    let page = call(server.port, &token,
        r#"{clipboardItems(offset:0,limit:50,query:"%_") {id text source label sensitive createdAt} clipboardItemCount(query:"%_")}"#).await;
    assert!(page.get("errors").is_none(), "{page}");
    assert_eq!(page["data"]["clipboardItemCount"], 1);
    let item = &page["data"]["clipboardItems"][0];
    assert_eq!(item["text"], "100%_ 🌍");
    assert_eq!(item["source"], "");
    assert_eq!(item["label"], "");
    assert_eq!(item["sensitive"], true);
    assert!(chrono::DateTime::parse_from_rfc3339(item["createdAt"].as_str().unwrap()).is_ok());
    bridge
        .publish_text("CLIPBOARD_REPORTED", payload)
        .await
        .unwrap();
    assert!(events.try_recv().is_err());
    for invalid in [
        json!({"text":" ", "sensitive":false}),
        json!({"text":"forged", "sensitive":false, "source":"peer"}),
    ] {
        assert!(
            bridge
                .publish_text("CLIPBOARD_REPORTED", invalid.to_string())
                .await
                .is_err()
        );
    }
    for invalid in [
        r#"mutation {deleteClipboardItems(query:" ") {affectedCount}}"#,
        r#"mutation {deleteClipboardItems(query:"source:peer") {affectedCount}}"#,
        r#"mutation {recordClipboard(input:{text:"forged",source:"peer",label:"",sensitive:false}) {inserted}}"#,
    ] {
        assert!(
            call(server.port, &token, invalid)
                .await
                .get("errors")
                .is_some()
        );
    }
    assert_eq!(
        call(
            server.port,
            &token,
            r#"mutation {deleteClipboardItems(query:"ids:missing") {affectedCount}}"#
        )
        .await["data"]["deleteClipboardItems"]["affectedCount"],
        0
    );
    server.shutdown().await;
    drop(bridge);
    let server = ContentServer::start(dir.path(), &token, prefs).unwrap();
    assert_eq!(
        call(server.port, &token, r#"{clipboardItemCount(query:"")}"#).await["data"]["clipboardItemCount"],
        1
    );
    assert_eq!(
        call(
            server.port,
            &token,
            r#"mutation {deleteClipboardItems(query:"all:true") {affectedCount}}"#
        )
        .await["data"]["deleteClipboardItems"]["affectedCount"],
        1
    );
    server.shutdown().await;
}
