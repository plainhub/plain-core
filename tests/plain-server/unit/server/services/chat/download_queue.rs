#![cfg(feature = "http_transport")]

use plain_rs::runtime::Runtime as ServerState;
use serde_json::{Value, json};
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, crate::server::ContentServer, ServerState) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state().core;
    let peer = plain_rs::db::DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    (dir, server, state)
}
fn seed_chat(state: &ServerState, id: &str, size: usize) {
    let row=plain_rs::db::DChat::new("peer","me","",&json!({"type":"FILES","value":{"items":[{"id":id,"uri":format!("fsid:{id}"),"fileName":"fixture.bin","size":size}]}}).to_string());
    let mut row = row;
    row.id = id.into();
    state.db.insert_chat(&row).unwrap();
}
async fn terminal(state: &ServerState, id: &str) -> Option<Value> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let row = state.downloads.snapshot()["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == id)
                .cloned();
            if row.as_ref().is_some_and(|row| row["status"] == "FAILED") {
                return row;
            }
            if row.is_none() {
                if let Some(chat) = plain_rs::db::chat_store::messages::get(&state.db, id).unwrap()
                {
                    let content: Value = serde_json::from_str(&chat.content).unwrap();
                    if content["value"]["items"][0]["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.starts_with("fid:"))
                    {
                        return None;
                    }
                }
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn root_ble_chunks_commit_exact_bytes_and_reject_short_and_overlong_reads() {
    let (dir, server, state) = fixture();
    let payload = Arc::new((0..16384).map(|n| (n % 251) as u8).collect::<Vec<_>>());
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let host = state.host.clone();
    let data = payload.clone();
    let ble = state.ble_transport.clone();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let plain_rs::protocols::ble::Request::FileChunk {
                        file_id,
                        offset,
                        length,
                        ..
                    } = ble.request_for_test(&request["params"])
                    else {
                        panic!("Expected file request")
                    };
                    assert_eq!(length, 8192);
                    let offset = offset as usize;
                    let id = file_id.as_str();
                    let bytes = if id == "short" {
                        b"short".as_slice()
                    } else if id == "overlong" {
                        b"overlong".as_slice()
                    } else {
                        &data[offset..data.len().min(offset + 8192)]
                    };
                    ble.reply_for_test(
                        &request["params"],
                        plain_rs::protocols::ble::response(200, bytes).unwrap(),
                    )
                }
                "peerTransportSocketCloseAll" => json!(true),
                _ => panic!("{request}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    seed_chat(&state, "exact", payload.len());
    let mut events = state.events.subscribe();
    let native = server.native_graphql().unwrap();
    let response = native
        .execute(async_graphql::Request::new(
            r#"mutation { enqueueChatDownload(messageId:"exact", fileId:"exact", peerId:"peer") }"#,
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["enqueueChatDownload"],
        true
    );

    assert!(terminal(&state, "exact").await.is_none());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut updated = false;
        let mut retired = false;
        while !updated || !retired {
            let event = events.recv().await.unwrap();
            let payload: Value = serde_json::from_str(&event.payload).unwrap_or(Value::Null);
            if event.event_type == plain_rs::chat::events::WS_MESSAGE_UPDATED
                && payload[0]["id"] == "exact"
            {
                let content: Value =
                    serde_json::from_str(payload[0]["content"].as_str().unwrap()).unwrap();
                assert!(
                    content["value"]["items"][0]["uri"]
                        .as_str()
                        .unwrap()
                        .starts_with("fid:")
                );
                updated = true;
            }
            if event.event_type == plain_rs::chat::events::WS_DOWNLOAD_PROGRESS
                && payload.as_array().is_some_and(|items| items.is_empty())
            {
                retired = true;
            }
        }
    })
    .await
    .unwrap();
    let response = native.execute(async_graphql::Request::new("{ chatDownloadQueue { runtimeId revision tasks { id status downloaded file { id: fileId uri durationMs } peer { id createdAt updatedAt } } } }")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let projected = response.data.into_json().unwrap();
    assert_eq!(
        projected["chatDownloadQueue"]["runtimeId"],
        state.downloads.snapshot()["runtimeId"]
    );
    assert!(
        projected["chatDownloadQueue"]["tasks"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let chat = plain_rs::db::chat_store::messages::get(&state.db, "exact")
        .unwrap()
        .unwrap();
    let content: Value = serde_json::from_str(&chat.content).unwrap();
    let uri = content["value"]["items"][0]["uri"].as_str().unwrap();
    assert!(uri.starts_with("fid:"));
    let hash = uri[4..].split('.').next().unwrap();
    let record = state.db.app_file_get(hash).unwrap().unwrap();
    assert_eq!(
        std::fs::read(dir.path().join(record.real_path)).unwrap(),
        *payload
    );
    for (id, size) in [("short", 20), ("overlong", 3)] {
        seed_chat(&state, id, size);
        state.downloads.enqueue(id, id, "peer").unwrap();
        let row = terminal(&state, id).await.unwrap();
        assert_eq!(row["status"], "FAILED");
        assert!(
            !dir.path()
                .join("attachment-transfers")
                .join(row["generation"].as_str().unwrap())
                .exists()
        );
        let row = plain_rs::db::chat_store::messages::get(&state.db, id)
            .unwrap()
            .unwrap();
        assert!(row.content.contains("fsid:"));
    }
    server.shutdown().await;
    state.host.disconnect(generation);
    responder.abort();
}

#[tokio::test]
async fn explicit_download_retry_roots_probe_open_transport_and_commit_exact_bytes() {
    use plain_rs::transport::router::{Outcome, TransportType};
    use std::sync::atomic::{AtomicUsize, Ordering};
    for (root, fresh) in [
        ("retryChatDownload(id:\"retry\")", false),
        ("retryDownload(messageId:\"retry\",peerId:\"peer\")", false),
        ("retryDownload(messageId:\"retry\",peerId:\"peer\")", true),
    ] {
        let (dir, server, state) = fixture();
        let peer = plain_rs::db::chat_store::peers::get(&state.db, "peer")
            .unwrap()
            .unwrap();
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
        let responder = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let result = match request["method"].as_str().unwrap() {
                    "peerTransportCapabilities" => json!(["BLE"]),
                    "peerTransportBleExchange" => {
                        let plain_rs::protocols::ble::Request::FileChunk {
                            file_id, offset, ..
                        } = ble.request_for_test(&request["params"])
                        else {
                            panic!("Expected file request")
                        };
                        assert_eq!(file_id, "retry");
                        assert_eq!(offset, 0);
                        count.fetch_add(1, Ordering::SeqCst);
                        ble.reply_for_test(
                            &request["params"],
                            plain_rs::protocols::ble::response(200, b"abc").unwrap(),
                        )
                    }
                    "peerTransportSocketCloseAll" => json!(true),
                    _ => panic!("{request}"),
                };
                host.reply(generation, json!({"id":request["id"],"result":result}))
                    .unwrap();
            }
        });
        seed_chat(&state, "retry", 3);
        if !fresh {
            assert!(state.downloads.enqueue("retry", "retry", "peer").unwrap());
            let failed = terminal(&state, "retry").await.unwrap();
            assert_eq!(
                failed["error"],
                "All peer transports unavailable or temporarily blocked"
            );
            assert_eq!(exchanges.load(Ordering::SeqCst), 0);
        }
        let native = server.native_graphql().unwrap();
        let response = native
            .execute(async_graphql::Request::new(format!("mutation {{{root}}}")))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert!(terminal(&state, "retry").await.is_none());
        assert_eq!(exchanges.load(Ordering::SeqCst), 1);
        let chat = plain_rs::db::chat_store::messages::get(&state.db, "retry")
            .unwrap()
            .unwrap();
        let content: Value = serde_json::from_str(&chat.content).unwrap();
        let uri = content["value"]["items"][0]["uri"].as_str().unwrap();
        let hash = uri.strip_prefix("fid:").unwrap().split('.').next().unwrap();
        let record = state.db.app_file_get(hash).unwrap().unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(record.real_path)).unwrap(),
            b"abc"
        );
        state.host.disconnect(generation);
        responder.abort();
        server.shutdown().await;
    }
}
