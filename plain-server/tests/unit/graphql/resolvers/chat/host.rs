use crate::server::ContentServer;
use async_graphql::Request;
use serde_json::{Value, json};
use std::sync::Arc;

fn server(dir: &std::path::Path) -> ContentServer {
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.join("prefs.json")).unwrap());
    ContentServer::start(
        &dir,
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap()
}
async fn execute(server: &ContentServer, document: String) -> Value {
    let result = server
        .graphql_executor()
        .execute(Request::new(document))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    result.data.into_json().unwrap()
}
#[tokio::test]
async fn local_messages_edit_forward_and_clear_through_native_executor() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    let result = execute(
        &server,
        r#"mutation { sendChatItem(target:"peer:local", content:"{\"type\":\"TEXT\",\"value\":{\"text\":\"original\"}}") {id status content} }"#
            .into(),
    )
    .await;
    let id = result["sendChatItem"][0]["id"].as_str().unwrap();
    assert_eq!(result["sendChatItem"][0]["status"], "SENT");
    assert_eq!(
        events.recv().await.unwrap().event_type,
        plain_rs::chat::events::WS_MESSAGE_CREATED
    );
    let edited = execute(
        &server,
        format!(
            r#"mutation {{ editChatText(id:{}, text:"edited") {{changed chat {{content}}}} }}"#,
            json!(id)
        ),
    )
    .await;
    assert_eq!(edited["editChatText"]["changed"], true);
    assert_eq!(
        serde_json::from_str::<Value>(edited["editChatText"]["chat"]["content"].as_str().unwrap())
            .unwrap()["value"]["text"],
        "edited"
    );
    execute(
        &server,
        format!(
            r#"mutation {{ forwardChatItems(id:{}, targets:["peer:local"]) {{id status}} }}"#,
            json!(id)
        ),
    )
    .await;
    let rows = execute(&server, r#"{ chatRecords(query:"peer:local text:edited sort:desc", offset:0, limit:20) {id} chatCount(query:"peer:local") chatConversations {id chat {id}} }"#.into()).await;
    assert_eq!(rows["chatCount"], 2);
    assert_eq!(rows["chatRecords"].as_array().unwrap().len(), 2);
    assert_eq!(rows["chatConversations"][0]["id"], "local");
    let unrelated = plain_rs::chat::messages::message_lifecycle::create(
        &state.db,
        "unrelated-peer",
        "",
        r#"{"type":"TEXT","value":{"text":"keep"}}"#,
    )
    .unwrap();
    for query in ["", "  ", "unknown:value", "text:edited"] {
        let result = execute(
            &server,
            format!(
                "mutation {{ deleteChatItems(query:{}) {{affectedCount}} }}",
                json!(query)
            ),
        )
        .await;
        assert_eq!(result["deleteChatItems"]["affectedCount"], 0);
        assert_eq!(state.db.get_all_chats().len(), 3);
    }
    let cleared = execute(
        &server,
        r#"mutation { deleteChatItems(query:"peer:=local") {affectedCount} }"#.into(),
    )
    .await;
    assert_eq!(cleared["deleteChatItems"]["affectedCount"], 2);
    let remaining = state.db.get_all_chats();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, unrelated.id);
    server.shutdown().await;
}
#[tokio::test]
async fn prepared_files_retain_metadata_and_message_owners_after_import_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let source = dir.path().join("fixture.txt");
    std::fs::write(&source, b"abc").unwrap();
    let store = plain_rs::files::app_files::FileStore::new(state.db.clone(), dir.path().into());
    let file = store
        .import(source, "fixture.txt".into(), "text/plain".into(), false)
        .await
        .unwrap();
    let suffix = file.real_path.rsplit('/').next().unwrap();
    let content = json!({"type":"FILES","value":{"items":[{"id":"attachment","uri":format!("fid:{suffix}"),"size":3,"fileName":"fixture.txt","width":12,"height":34,"durationMs":56,"summary":""}]}}).to_string();
    let first = execute(
        &server,
        format!(
            r#"mutation {{ sendChatItem(target:"local", content:{}) {{id content status}} }}"#,
            json!(content)
        ),
    )
    .await;
    let id = first["sendChatItem"][0]["id"].as_str().unwrap();
    execute(
        &server,
        format!(
            r#"mutation {{ forwardChatItems(id:{}, targets:["local"]) {{id}} }}"#,
            json!(id)
        ),
    )
    .await;
    let forwarded = execute(
        &server,
        format!(r#"mutation {{ forwardChatItems(id:{}, targets:["local","peer:local"]) {{id content}} }}"#, json!(id)),
    ).await;
    for row in forwarded["forwardChatItems"].as_array().unwrap() {
        assert_eq!(row["content"], first["sendChatItem"][0]["content"]);
    }
    store.release(file.id.clone()).await.unwrap();
    assert_eq!(
        state.db.app_file_get(&file.id).unwrap().unwrap().ref_count,
        4
    );
    let saved: Value =
        serde_json::from_str(first["sendChatItem"][0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(saved["value"]["items"][0]["width"], 12);
    assert_eq!(saved["value"]["items"][0]["durationMs"], 56);
    execute(
        &server,
        r#"mutation { deleteChatItems(query:"peer:=local") {affectedCount} }"#.into(),
    )
    .await;
    assert!(state.db.app_file_get(&file.id).unwrap().is_none());
    server.shutdown().await;
}
#[tokio::test]
async fn long_text_files_and_invalid_targets_use_the_same_message_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(dir.path());
    let text = "😀".repeat(1100);
    let result = execute(
        &server,
        format!(
            r#"mutation {{sendChatItem(target:"local", content:{}) {{content}}}}"#,
            json!(json!({"type":"TEXT","value":{"text":text}}).to_string())
        ),
    )
    .await;
    let content: Value =
        serde_json::from_str(result["sendChatItem"][0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["type"], "FILES");
    assert_eq!(content["value"]["items"][0]["size"], text.len());
    let rejected = server
        .graphql_executor()
        .execute(Request::new(
            r#"mutation { sendChatItem(target:"channel:", content:"{\"type\":\"TEXT\",\"value\":{\"text\":\"bad\"}}") {id} }"#,
        ))
        .await;
    assert!(!rejected.errors.is_empty());
    assert_eq!(server.runtime_state().db.get_all_chats().len(), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn retry_returns_pending_before_platform_transport_completes() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(dir.path());
    let (kp, pk) = plain_rs::crypto::ed25519_generate();
    let state = server.runtime_state();
    state.prefs.set("client_id", "local").unwrap();
    state
        .prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&kp),
        )
        .unwrap();
    let mut remote = plain_rs::db::DPeer::new(
        "remote",
        "remote",
        "",
        Some(443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    remote.status = plain_rs::chat::enums::PeerStatus::Paired;
    remote.key = plain_rs::utils::base64::base64_encode(&[7; 32]);
    remote.public_key = plain_rs::utils::base64::base64_encode(&pk);
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[remote],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let row = plain_rs::chat::messages::message_lifecycle::create(
        &state.db,
        "remote",
        "",
        r#"{"type":"TEXT","value":{"text":"retry"}}"#,
    )
    .unwrap();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let result = execute(
        &server,
        format!(
            r#"mutation {{retryChatItem(id:{}) {{status}}}}"#,
            json!(row.id)
        ),
    )
    .await;
    assert_eq!(result["retryChatItem"]["status"], "PENDING");
    let caps = requests.recv().await.unwrap();
    state
        .host
        .reply(generation, json!({"id":caps["id"],"result":["BLE"]}))
        .unwrap();
    let exchange = requests.recv().await.unwrap();
    let result = plain_rs::services::peers::ble_transport::test_reply(
        &state.ble_transport,
        &exchange["params"],
        json!({"data":{"createChatItem":[]}}),
        &[7; 32],
    );
    state
        .host
        .reply(generation, json!({"id":exchange["id"],"result":result}))
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if plain_rs::db::chat_store::messages::get(&state.db, &row.id)
                .unwrap()
                .unwrap()
                .status
                == plain_rs::chat::enums::ChatStatus::Sent
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    state.host.disconnect(generation);
    server.shutdown().await;
}

#[tokio::test]
async fn batch_forward_uses_current_content_and_owns_each_creation_event() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let source = execute(
        &server,
        r#"mutation { sendChatItem(target:"local", content:"{\"type\":\"TEXT\",\"value\":{\"text\":\"old\"}}") {id} }"#.into(),
    )
    .await;
    let id = source["sendChatItem"][0]["id"].as_str().unwrap();
    execute(
        &server,
        format!(
            r#"mutation {{ editChatText(id:{}, text:"current") {{changed}} }}"#,
            json!(id)
        ),
    )
    .await;
    let mut events = state.events.subscribe();
    let (_generation, mut host_requests) = crate::test_support::HostRequests::connect(&state.host);
    let result = execute(&server, format!(r#"mutation {{ forwardChatItems(id:{}, targets:["local","peer:local"]) {{id content status}} }}"#, json!(id))).await;
    let rows = result["forwardChatItems"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0]["id"], rows[1]["id"]);
    for row in rows {
        assert_eq!(row["status"], "SENT");
        assert_eq!(
            serde_json::from_str::<Value>(row["content"].as_str().unwrap()).unwrap()["value"]["text"],
            "current"
        );
    }
    let mut created = 0;
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CONTENT_CHANGED");
        created += usize::from(event.event_type == plain_rs::chat::events::WS_MESSAGE_CREATED);
    }
    assert_eq!(created, 2);
    assert!(host_requests.try_recv().is_err());
    for (source, targets) in [
        (id, vec!["local", "channel:"]),
        ("missing", vec!["local"]),
        (id, vec!["local"; 129]),
    ] {
        let result = server
            .graphql_executor()
            .execute(Request::new(format!(
                "mutation {{ forwardChatItems(id:{}, targets:{}) {{id}} }}",
                json!(source),
                json!(targets)
            )))
            .await;
        assert!(!result.errors.is_empty());
    }
    let empty = execute(
        &server,
        r#"mutation { forwardChatItems(id:"missing", targets:[]) {id} }"#.into(),
    )
    .await;
    assert_eq!(empty["forwardChatItems"], json!([]));
    assert_eq!(state.db.get_all_chats().len(), 3);
    assert!(events.try_recv().is_err());
    server.shutdown().await;
}
