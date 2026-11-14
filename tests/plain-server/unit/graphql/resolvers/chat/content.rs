use crate::server::ContentServer;
use plain_graphql::{Request, Variables};
use plain_libs::serde_json::{Value, json};
use std::sync::Arc;

fn server(path: &std::path::Path) -> ContentServer {
    let prefs = Arc::new(plain_rs::features::prefs::Prefs::load(&path.join("prefs.json")).unwrap());
    ContentServer::start(
        path,
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap()
}
const SEND: &str = "mutation($content:ChatContentInput!){sendChatItems(targets:[\"peer:local\"],content:$content){id status content{... on ChatTextContent{__typename text} ... on ChatImagesContent{__typename items{id size durationMs}} ... on ChatFilesContent{__typename items{id size durationMs}} ... on ChatShareContent{__typename shareId}} deliveryResults{peerId peerName error}}}";

#[tokio::test]
async fn typed_content_roundtrips_all_kinds_without_changing_stored_envelopes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    let sdl = server.runtime_state().public.sdl();
    assert!(sdl.contains("input ChatContentInput @oneOf"));
    let item = sdl
        .split("type ChatItem {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(item.contains("content: ChatContent!"));
    assert!(item.contains("deliveryResults: [ChatDeliveryResult!]!"));
    assert!(!item.contains("statusData"));
    for (input, kind, typename) in [
        (json!({"text":{"text":"Hello"}}), "TEXT", "ChatTextContent"),
        (
            json!({"images":{"items":[]}}),
            "IMAGES",
            "ChatImagesContent",
        ),
        (json!({"files":{"items":[]}}), "FILES", "ChatFilesContent"),
        (
            json!({"share":{"shareId":"share","urlToken":"token","peerInfo":{"id":"peer","ip":"example.org","port":443},"name":"Share","itemCount":0,"totalSize":0}}),
            "SHARE",
            "ChatShareContent",
        ),
    ] {
        let response = server
            .graphql_executor()
            .execute(Request::new(SEND).variables(Variables::from_json(json!({"content":input.clone()}))))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        let item = &data["sendChatItems"][0];
        assert_eq!(item["content"]["__typename"], typename);
        assert_eq!(item["deliveryResults"], json!([]));
        let row = server
            .runtime_state()
            .db
            .get_chat_by_id(item["id"].as_str().unwrap())
            .unwrap();
        let stored: Value = plain_libs::serde_json::from_str(&row.content).unwrap();
        assert_eq!(stored["type"], kind);
        assert!(stored["value"].is_object());
    }
    server.shutdown().await;
}

#[tokio::test]
async fn invalid_oneof_and_legacy_string_inputs_are_rejected_before_effects() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    for input in [
        json!({}),
        json!({"text":null}),
        json!({"unknown":{"text":"invalid"}}),
        json!({"text":{"text":"ok"},"files":{"items":[]}}),
        json!({"text":{"text":42}}),
        json!("{\"type\":\"TEXT\",\"value\":{\"text\":\"legacy\"}}"),
        json!({"files":{"items":[{"uri":"fixture","size":-1}]}}),
        json!({"files":{"items":[{"uri":"fixture","size":0}]}}),
        json!({"files":{"items":[{"id":null,"uri":"fixture","size":0}]}}),
        json!({"files":{"items":[{"id":false,"uri":"fixture","size":0}]}}),
        json!({"files":{"items":[{"id":"","uri":"fixture","size":0}]}}),
        json!({"images":{"items":[{"id":"","uri":"fixture","size":0}]}}),
    ] {
        let response = server
            .graphql_executor()
            .execute(Request::new(SEND).variables(Variables::from_json(json!({"content":input.clone()}))))
            .await;
        assert!(!response.errors.is_empty(), "accepted invalid input: {input}");
        assert!(server.runtime_state().db.get_all_chats().is_empty());
    }
    server.shutdown().await;
}

#[tokio::test]
async fn delivery_results_preserve_success_failure_and_empty_information() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    let db = server.runtime_state().db.clone();
    let row = plain_rs::features::chat::messages::message_lifecycle::create(
        &db,
        "local",
        "",
        r#"{"type":"TEXT","value":{"text":"Hello"}}"#,
    )
    .unwrap();
    let results = vec![
        plain_rs::features::chat::channels::chat_helper::ChannelDeliveryResult {
            peer_id: "one".into(),
            peer_name: "One".into(),
            error: None,
        },
        plain_rs::features::chat::channels::chat_helper::ChannelDeliveryResult {
            peer_id: "two".into(),
            peer_name: "Two".into(),
            error: Some("offline".into()),
        },
    ];
    db.update_chat_status_and_data(
        &row.id,
        plain_rs::features::chat::enums::ChatStatus::Partial,
        &plain_rs::features::chat::channels::chat_helper::build_status_data_json(&results),
    )
    .unwrap();
    let response = server
        .graphql_executor()
        .execute(
            Request::new(
                "query($id:ID!){chatItem(id:$id){status deliveryResults{peerId peerName error}}}",
            )
            .variables(Variables::from_json(json!({"id":row.id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["chatItem"]["status"], "PARTIAL");
    assert_eq!(
        data["chatItem"]["deliveryResults"],
        json!([{"peerId":"one","peerName":"One","error":null},{"peerId":"two","peerName":"Two","error":"offline"}])
    );
    db.update_chat_status_and_data(
        &row.id,
        plain_rs::features::chat::enums::ChatStatus::Failed,
        "",
    )
    .unwrap();
    let response = server
        .graphql_executor()
        .execute(
            Request::new("query($id:ID!){chatItem(id:$id){status deliveryResults{peerId error}}}")
                .variables(Variables::from_json(json!({"id":row.id}))),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["chatItem"]["status"], "FAILED");
    assert_eq!(data["chatItem"]["deliveryResults"], json!([]));
    server.shutdown().await;
}
