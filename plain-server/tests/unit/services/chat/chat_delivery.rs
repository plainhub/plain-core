#![cfg(feature = "http_transport")]
use serde_json::json;
use super::*;
use serde_json::Value;
use std::sync::Arc;
#[tokio::test]
async fn host_transport_preserves_signed_wire_and_requires_an_actual_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "self").unwrap();
    let server = crate::assembly::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let host = state.host.clone();
    let (generation, mut receiver) = crate::test_support::HostRequests::connect(&host);
    let transport = Transport {
        state: state.clone(),
        attempt: plain_rs::transport::router::Attempt::Automatic,
    };
    let mut peer = DPeer::new(
        "fixture",
        "fixture",
        "",
        Some(443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    peer.key = plain_rs::utils::base64::base64_encode(&[7; 32]);
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let responder = {
        let host = host.clone();
        let ble = state.ble_transport.clone();
        tokio::spawn(async move {
            for (index, data) in [
                json!({"createChatItem":[]}),
                json!({"createChatItem":null}),
                json!({"createChatItem":[]}),
            ].into_iter().enumerate() {
                let capabilities = receiver.recv().await.unwrap();
                assert_eq!(capabilities["method"], "peerTransportCapabilities");
                host.reply(
                    generation,
                    json!({"id":capabilities["id"],"result":["BLE"]}),
                )
                .unwrap();
                let request = receiver.recv().await.unwrap();
                assert_eq!(request["method"], "peerTransportBleExchange");
                assert_eq!(request["params"]["peer"]["id"], "fixture");
                assert!(
                    matches!(ble.request_for_test(&request["params"]), plain_rs::protocols::ble::Request::PeerGraphql { channel_id, .. } if channel_id == "channel")
                );
                assert_eq!(
                    crate::http::ble::test_peer_wire(
                        &ble,
                        &request["params"],
                        &[7; 32]
                    ),
                    "signed|123|request"
                );
                let errors = if data == json!({"createChatItem":null}) {
                    Value::Null
                } else if data == json!({"createChatItem":[]}) && index == 2
                {
                    json!([{"message":"rejected"}])
                } else {
                    Value::Null
                };
                host.reply(
                    generation,
                    json!({"id":request["id"],"result":crate::http::ble::test_reply(&ble, &request["params"], json!({"data":data,"errors":errors}),&[7;32])}),
                )
                .unwrap();
            }
        })
    };
    assert!(
        transport
            .message(&peer, "self", "channel", &[7; 32], "signed|123|request")
            .await
            .is_ok()
    );
    assert!(
        transport
            .message(&peer, "self", "channel", &[7; 32], "signed|123|request")
            .await
            .is_err()
    );
    assert!(
        transport
            .message(&peer, "self", "channel", &[7; 32], "signed|123|request")
            .await
            .is_err()
    );
    responder.await.unwrap();
    host.disconnect(generation);
}

#[tokio::test]
async fn explicit_message_retry_root_probes_open_transport_and_sends_original_row_once() {
    use plain_rs::chat::enums::{ChatStatus, PeerStatus};
    use plain_rs::transport::router::{Outcome, TransportType};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "self").unwrap();
    let server = crate::assembly::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let mut peer = DPeer::new(
        "fixture",
        "fixture",
        "",
        Some(443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    peer.key = plain_rs::utils::base64::base64_encode(&[7; 32]);
    peer.status = PeerStatus::Paired;
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let content = json!({"type":"TEXT","value":{"text":"explicit retry fixture"}}).to_string();
    let mut original = plain_rs::db::DChat::new("me", "fixture", "", &content);
    original.id = "retry-native".into();
    original.status = ChatStatus::Failed;
    state.db.insert_chat(&original).unwrap();
    for _ in 0..2 {
        let ticket = state
            .transport
            .begin(&peer, &[TransportType::Ble])
            .unwrap()
            .ticket
            .unwrap();
        state
            .transport
            .finish(
                &ticket,
                Outcome::Unavailable {
                    error: "offline".into(),
                },
            )
            .unwrap();
    }
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let host = state.host.clone();
    let ble = state.ble_transport.clone();
    let exchanges = Arc::new(AtomicUsize::new(0));
    let count = exchanges.clone();
    let expected = content.clone();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let wire = crate::http::ble::test_peer_wire(
                        &ble,
                        &request["params"],
                        &[7; 32],
                    );
                    let document: Value =
                        serde_json::from_str(wire.splitn(3, '|').nth(2).unwrap()).unwrap();
                    assert_eq!(
                        document["query"],
                        plain_rs::transport::contract::CREATE_CHAT_ITEM
                    );
                    assert_eq!(document["variables"]["content"], expected);
                    count.fetch_add(1, Ordering::SeqCst);
                    crate::http::ble::test_reply(
                        &ble,
                        &request["params"],
                        json!({"data":{"createChatItem":[]}}),
                        &[7; 32],
                    )
                }
                "peerTransportSocketCloseAll" => json!(true),
                _ => panic!("{request}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    let automatic = deliver(&state, &original.id, None)
        .await
        .unwrap()
        .chat
        .unwrap();
    assert_eq!(automatic.status, ChatStatus::Failed);
    assert!(automatic.status_data.as_deref().is_some_and(|value| {
        value.contains("All peer transports unavailable or temporarily blocked")
    }));
    assert_eq!(exchanges.load(Ordering::SeqCst), 0);
    assert!(
        state
            .transport
            .begin(&peer, &[TransportType::Ble])
            .unwrap()
            .ticket
            .is_none()
    );
    let native = server.native_graphql().unwrap();
    let response = native
        .execute(async_graphql::Request::new(
            r#"mutation{retryChatItem(id:"retry-native"){id}}"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["retryChatItem"]["id"],
        original.id
    );
    let sent = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let row = plain_rs::db::chat_store::messages::get(&state.db, &original.id)
                .unwrap()
                .unwrap();
            if row.status == ChatStatus::Sent {
                break row;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(sent.id, original.id);
    assert_eq!(sent.content, content);
    assert_eq!(exchanges.load(Ordering::SeqCst), 1);
    let recovered = state
        .transport
        .begin(&peer, &[TransportType::Ble])
        .unwrap()
        .ticket
        .unwrap();
    state.transport.abort(&recovered);
    state.host.disconnect(generation);
    responder.abort();
    server.shutdown().await;
}
