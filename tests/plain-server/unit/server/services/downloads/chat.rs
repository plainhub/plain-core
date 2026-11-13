#![cfg(feature = "http_transport")]

use plain_libs::serde_json::{Value, json};
use plain_rs::runtime::Runtime as ServerState;
use std::sync::Arc;

fn fixture() -> (
    plain_libs::temporary::TempDir,
    crate::server::ContentServer,
    ServerState,
) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state().core;
    let peer = plain_rs::infrastructure::db::DPeer::new(
        "peer",
        "fixture",
        "",
        Some(443),
        plain_rs::features::chat::enums::DeviceType::Phone,
    );
    plain_rs::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[peer],
        plain_rs::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    (dir, server, state)
}
fn seed_chat(state: &ServerState, id: &str, size: usize) {
    let row=plain_rs::infrastructure::db::DChat::new("peer","me","",&json!({"type":"FILES","value":{"items":[{"id":id,"uri":format!("fsid:{id}"),"fileName":"fixture.bin","size":size}]}}).to_string());
    let mut row = row;
    row.id = id.into();
    state.db.insert_chat(&row).unwrap();
}
async fn enqueue(
    server: &crate::server::ContentServer,
    message_id: &str,
    file_ids: &[&str],
) -> String {
    let response = server.native_graphql().unwrap().execute(plain_graphql::Request::new(
        "mutation($input: DownloadInput!) { createDownload(input: $input) { id } }"
    ).variables(plain_graphql::Variables::from_json(json!({"input":{"chat":{"messageId":message_id,"peerId":"peer","fileIds":file_ids}}})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()["createDownload"]["id"]
        .as_str()
        .unwrap()
        .into()
}

async fn terminal(state: &ServerState, message_id: &str) -> Option<Value> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let row = state.downloads.snapshot().into_iter().find(|row| matches!(&row.source, plain_rs::features::downloads::Source::Chat {message_id: id, ..} if id == message_id));
            if let Some(row) = row {
                if row.status == plain_rs::features::downloads::Status::Completed { return None; }
                if matches!(row.status, plain_rs::features::downloads::Status::Failed | plain_rs::features::downloads::Status::Partial) { return Some(plain_libs::serde_json::to_value(row).unwrap()); }
            }
            tokio::task::yield_now().await;
        }
    }).await.unwrap()
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
                    let plain_rs::infrastructure::protocols::ble::Request::FileChunk {
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
                    let bytes = if id == "sql-failure" {
                        b"new".as_slice()
                    } else if id == "short" {
                        b"short".as_slice()
                    } else if id == "overlong" {
                        b"overlong".as_slice()
                    } else {
                        &data[offset..data.len().min(offset + 8192)]
                    };
                    let status = if bytes.is_empty() { 416 } else { 206 };
                    ble.reply_for_test(
                        &request["params"],
                        plain_rs::infrastructure::protocols::ble::response(status, bytes).unwrap(),
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
    let download_id = enqueue(&server, "exact", &["exact"]).await;

    assert!(terminal(&state, "exact").await.is_none());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut updated = false;
        let mut completed = false;
        while !updated || !completed {
            let event = events.recv().await.unwrap();
            if !matches!(
                event.event_type,
                plain_rs::features::chat::events::WS_MESSAGE_UPDATED
                    | plain_rs::features::chat::events::WS_DOWNLOAD_PROGRESS
            ) {
                continue;
            }
            let payload: Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
            if event.event_type == plain_rs::features::chat::events::WS_MESSAGE_UPDATED
                && payload[0]["id"] == "exact"
            {
                let content: Value =
                    plain_libs::serde_json::from_str(payload[0]["content"].as_str().unwrap())
                        .unwrap();
                assert!(
                    content["value"]["items"][0]["uri"]
                        .as_str()
                        .unwrap()
                        .starts_with("fid:")
                );
                updated = true;
            }
            if event.event_type == plain_rs::features::chat::events::WS_DOWNLOAD_PROGRESS {
                assert_eq!(payload, true);
                completed = state.downloads.snapshot().iter().any(|task| {
                    task.id == download_id
                        && task.status == plain_rs::features::downloads::Status::Completed
                });
            }
        }
    })
    .await
    .unwrap();
    let response = native.execute(plain_graphql::Request::new("{ downloads { id status downloadedBytes source { ... on ChatDownloadSource { messageId fileIds } } } }")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let projected = response.data.into_json().unwrap();
    assert_eq!(projected["downloads"][0]["id"], download_id);
    assert_eq!(projected["downloads"][0]["status"], "COMPLETED");
    assert_eq!(
        projected["downloads"][0]["source"]["fileIds"],
        json!(["exact"])
    );

    let chat = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, "exact")
        .unwrap()
        .unwrap();
    let content: Value = plain_libs::serde_json::from_str(&chat.content).unwrap();
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
        enqueue(&server, id, &[id]).await;
        let row = terminal(&state, id).await.unwrap();
        assert_eq!(row["status"], "FAILED");
        assert_eq!(
            std::fs::read_dir(dir.path().join("attachment-transfers"))
                .unwrap()
                .count(),
            0
        );
        let row = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, id)
            .unwrap()
            .unwrap();
        assert!(row.content.contains("fsid:"));
    }
    seed_chat(&state, "sql-failure", 3);
    state.db.with_conn(|connection| connection.execute_batch("CREATE TRIGGER reject_download BEFORE INSERT ON app_files BEGIN SELECT RAISE(ABORT,'controlled import failure'); END;")).unwrap();
    enqueue(&server, "sql-failure", &["sql-failure"]).await;
    let failed = terminal(&state, "sql-failure").await.unwrap();
    assert_eq!(failed["status"], "FAILED");
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .contains("controlled import failure")
    );
    let row = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, "sql-failure")
        .unwrap()
        .unwrap();
    assert!(row.content.contains("fsid:sql-failure"));
    assert_eq!(
        std::fs::read_dir(dir.path().join("attachment-transfers"))
            .unwrap()
            .count(),
        0
    );
    server.shutdown().await;
    state.host.disconnect(generation);
    responder.abort();
}

#[tokio::test]
async fn explicit_download_retry_roots_probe_open_transport_and_commit_exact_bytes() {
    use plain_rs::infrastructure::transport::router::{Outcome, TransportType};
    use std::sync::atomic::{AtomicUsize, Ordering};
    for kind in ["FILES", "IMAGES"] {
        let payload = if kind == "IMAGES" {
            plain_libs::utils::base64::base64_decode_checked("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a7X8AAAAASUVORK5CYII=").unwrap()
        } else {
            b"abc".to_vec()
        };
        let (dir, server, state) = fixture();
        let peer = plain_rs::infrastructure::db::chat_store::peers::get(&state.db, "peer")
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
        let response_bytes = payload.clone();
        let responder = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let result = match request["method"].as_str().unwrap() {
                    "peerTransportCapabilities" => json!(["BLE"]),
                    "peerTransportBleExchange" => {
                        let plain_rs::infrastructure::protocols::ble::Request::FileChunk {
                            file_id,
                            offset,
                            ..
                        } = ble.request_for_test(&request["params"])
                        else {
                            panic!("Expected file request")
                        };
                        assert_eq!(file_id, "retry");
                        assert_eq!(offset, 0);
                        count.fetch_add(1, Ordering::SeqCst);
                        ble.reply_for_test(
                            &request["params"],
                            plain_rs::infrastructure::protocols::ble::response(
                                206,
                                &response_bytes,
                            )
                            .unwrap(),
                        )
                    }
                    "peerTransportSocketCloseAll" => json!(true),
                    _ => panic!("{request}"),
                };
                host.reply(generation, json!({"id":request["id"],"result":result}))
                    .unwrap();
            }
        });
        seed_chat(&state, "retry", payload.len());
        if kind == "IMAGES" {
            let mut row =
                plain_rs::infrastructure::db::chat_store::messages::get(&state.db, "retry")
                    .unwrap()
                    .unwrap();
            let mut content: Value = plain_libs::serde_json::from_str(&row.content).unwrap();
            content["type"] = json!("IMAGES");
            content["value"]["items"][0]["fileName"] = json!("fixture.png");
            row.content = content.to_string();
            plain_rs::infrastructure::db::chat_store::messages::save(
                &state.db,
                &[row],
                plain_rs::infrastructure::db::chat_store::SaveMode::Update,
            )
            .unwrap();
        }
        let id = enqueue(&server, "retry", &["retry"]).await;
        let failed = terminal(&state, "retry").await.unwrap();
        assert_eq!(
            failed["error"],
            "All peer transports unavailable or temporarily blocked"
        );
        assert_eq!(exchanges.load(Ordering::SeqCst), 0);
        let native = server.native_graphql().unwrap();
        let response = native
            .execute(plain_graphql::Request::new(format!(
                "mutation {{ retryDownload(id:{}) }}",
                json!(id)
            )))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert!(terminal(&state, "retry").await.is_none());
        assert_eq!(exchanges.load(Ordering::SeqCst), 1);
        let chat = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, "retry")
            .unwrap()
            .unwrap();
        let content: Value = plain_libs::serde_json::from_str(&chat.content).unwrap();
        let uri = content["value"]["items"][0]["uri"].as_str().unwrap();
        let hash = uri.strip_prefix("fid:").unwrap().split('.').next().unwrap();
        let record = state.db.app_file_get(hash).unwrap().unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(record.real_path)).unwrap(),
            payload
        );
        state.host.disconnect(generation);
        responder.abort();
        server.shutdown().await;
    }
}

#[tokio::test]
async fn grouped_files_and_images_retry_only_failed_attachments_and_preserve_completed_imports() {
    for kind in ["FILES", "IMAGES"] {
        let (directory, server, state) = fixture();
        let row = plain_rs::infrastructure::db::DChat::new(
            "peer",
            "me",
            "",
            &json!({"type":kind,"value":{"items":[
                {"id":"first","uri":"fsid:first","fileName":"first.bin","size":3},
                {"id":"second","uri":"fsid:second","fileName":"second.bin","size":3}
            ]}})
            .to_string(),
        );
        state.db.insert_chat(&row).unwrap();
        let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let observed = calls.clone();
        let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
        let host = state.host.clone();
        let ble = state.ble_transport.clone();
        let responder = tokio::spawn(async move {
            let mut fail_second = true;
            while let Some(request) = requests.recv().await {
                let result = match request["method"].as_str().unwrap() {
                    "peerTransportCapabilities" => json!(["BLE"]),
                    "peerTransportBleExchange" => {
                        let plain_rs::infrastructure::protocols::ble::Request::FileChunk {
                            file_id,
                            offset,
                            ..
                        } = ble.request_for_test(&request["params"])
                        else {
                            panic!("Expected attachment request")
                        };
                        assert_eq!(offset, 0);
                        observed.lock().unwrap().push(file_id.clone());
                        let bytes: &[u8] = match file_id.as_str() {
                            "first" => b"one",
                            "second" if fail_second => {
                                fail_second = false;
                                b"t"
                            }
                            "second" => b"two",
                            _ => panic!("Unexpected attachment {file_id}"),
                        };
                        ble.reply_for_test(
                            &request["params"],
                            plain_rs::infrastructure::protocols::ble::response(206, bytes).unwrap(),
                        )
                    }
                    "peerTransportSocketCloseAll" => json!(true),
                    _ => panic!("{request}"),
                };
                host.reply(generation, json!({"id":request["id"],"result":result}))
                    .unwrap();
            }
        });
        let id = enqueue(&server, &row.id, &["first", "second"]).await;
        let partial = terminal(&state, &row.id).await.unwrap();
        assert_eq!(partial["status"], "PARTIAL");
        assert_eq!(partial["doneFiles"], 1);
        assert_eq!(partial["downloadedBytes"], 3);
        let imported = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, &row.id)
            .unwrap()
            .unwrap();
        let content: Value = plain_libs::serde_json::from_str(&imported.content).unwrap();
        let first_uri = content["value"]["items"][0]["uri"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(first_uri.starts_with("fid:"));
        assert_eq!(content["value"]["items"][1]["uri"], "fsid:second");
        let response = server
            .native_graphql()
            .unwrap()
            .execute(
                plain_graphql::Request::new("mutation($id: ID!) { retryDownload(id: $id) }")
                    .variables(plain_graphql::Variables::from_json(json!({"id":id}))),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["retryDownload"], true);
        assert!(terminal(&state, &row.id).await.is_none());
        assert_eq!(*calls.lock().unwrap(), ["first", "second", "second"]);
        let imported = plain_rs::infrastructure::db::chat_store::messages::get(&state.db, &row.id)
            .unwrap()
            .unwrap();
        let content: Value = plain_libs::serde_json::from_str(&imported.content).unwrap();
        assert_eq!(content["value"]["items"][0]["uri"], first_uri);
        for (index, bytes) in [b"one", b"two"].iter().enumerate() {
            let uri = content["value"]["items"][index]["uri"].as_str().unwrap();
            let hash = uri.strip_prefix("fid:").unwrap().split('.').next().unwrap();
            let file = state.db.app_file_get(hash).unwrap().unwrap();
            assert_eq!(
                std::fs::read(directory.path().join(file.real_path)).unwrap(),
                *bytes
            );
        }
        let completed = &state.downloads.snapshot()[0];
        assert_eq!(completed.id, id);
        assert_eq!(
            (
                completed.total_files,
                completed.done_files,
                completed.downloaded_bytes
            ),
            (2, 2, 6)
        );
        assert_eq!(
            std::fs::read_dir(directory.path().join("attachment-transfers"))
                .unwrap()
                .count(),
            0
        );
        server.shutdown().await;
        state.host.disconnect(generation);
        responder.abort();
    }
}
