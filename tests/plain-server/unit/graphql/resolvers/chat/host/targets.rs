use super::*;

#[tokio::test]
async fn peer_targets_require_prefix_before_effects() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let content = json!({"type":"MESSAGE","value":{"format":"PLAIN","items":[],"text":"fixture"}}).to_string();
    let source = execute(&server, format!(
        "mutation {{ sendChatItems(targets:[\"peer:local\"],content:{}) {{id}} }}", content_input(&content)
    )).await;
    let id = json!(source["sendChatItems"][0]["id"].as_str().unwrap());
    let folder = json!(dir.path().to_string_lossy());
    for raw in ["local", "fixture-peer", "unknown:id", "", "peer:", "channel:"] {
        let targets = json!(["peer:local", raw]);
        let documents = [
            format!("mutation {{ sendChatItems(targets:{targets},content:{}) {{id}} }}", content_input(&content)),
            format!("mutation {{ forwardChatItems(id:{id},targets:{targets}) {{id}} }}"),
            format!("mutation {{ sendShare(shareId:\"missing\",targets:{targets}) {{id}} }}"),
            format!("mutation {{ sendFolderShare(path:{folder},name:\"fixture\",targets:{targets}) {{id}} }}"),
        ];
        for document in documents {
            let response = server.graphql_executor().execute(Request::new(&document)).await;
            assert!(!response.errors.is_empty(), "accepted {raw:?}: {document}");
            if !raw.is_empty() && raw != "peer:" && raw != "channel:" {
                assert!(response.errors.iter().any(|error| error.message.contains("Target must use peer:<id> or channel:<id>")), "{:?}", response.errors);
            }
            assert_eq!(state.db.get_all_chats().len(), 1, "partial send: {document}");
            assert!(state.db.share_list().unwrap().is_empty(), "partial share: {document}");
        }
    }
    let rows = execute(&server, "{ chatItems(query:\"peer:local\",offset:0,limit:10,sortBy:NEWEST_FIRST) {id toId} }".into()).await;
    assert_eq!(rows["chatItems"].as_array().unwrap().len(), 1);
    assert_eq!(rows["chatItems"][0]["toId"], "local");
    server.shutdown().await;
}

#[tokio::test]
async fn forwarding_a_note_preserves_the_source_and_returns_real_rows_per_target() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let (signing, _) = plain_libs::crypto::ed25519_generate();
    state
        .prefs
        .set(
            "signature_private_key",
            plain_libs::utils::base64::base64_encode(&signing[..32]),
        )
        .unwrap();
    let result = execute(
        &server,
        r#"mutation { sendChatItems(targets:["peer:local"], content:{message:{text:"**unchanged**",format:MARKDOWN}}) {id} }"#.into(),
    ).await;
    let source = &result["sendChatItems"][0]["id"];
    let operation = |note: &str| {
        format!(
            r#"mutation {{ forwardChatItems(id:{source},targets:["peer:local","peer:unavailable"],note:{}) {{
            id toId status deliveryResults {{peerId error}} content {{... on ChatMessageContent {{text format items {{type}}}}}}
        }} }}"#,
            json!(note)
        )
    };

    state.db.with_conn(|db| db.execute_batch(
        "CREATE TRIGGER fixture_abort_note BEFORE INSERT ON chats
         WHEN instr(NEW.content,'abort note')>0 BEGIN SELECT RAISE(ABORT,'fixture note failure'); END;"
    )).unwrap();
    let failed = server
        .graphql_executor()
        .execute(Request::new(operation("abort note")))
        .await;
    assert!(
        failed
            .errors
            .iter()
            .any(|e| e.message.contains("fixture note failure"))
    );
    assert_eq!(state.db.get_all_chats().len(), 1, "no partial creation");
    state
        .db
        .with_conn(|db| db.execute_batch("DROP TRIGGER fixture_abort_note;"))
        .unwrap();

    let result = execute(&server, operation("  separate note  ")).await;
    let rows = result["forwardChatItems"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    for (offset, target, status) in [(0, "local", "SENT"), (2, "unavailable", "FAILED")] {
        assert_eq!(rows[offset]["content"]["text"], "**unchanged**");
        assert_eq!(rows[offset]["content"]["format"], "MARKDOWN");
        assert_eq!(rows[offset + 1]["content"]["text"], "separate note");
        assert_eq!(rows[offset + 1]["content"]["format"], "PLAIN");
        for row in &rows[offset..offset + 2] {
            assert_eq!(row["toId"], target);
            assert_eq!(row["status"], status);
            assert_ne!(&row["id"], source);
        }
        assert_ne!(rows[offset]["id"], rows[offset + 1]["id"]);
    }
    assert_eq!(rows[3]["deliveryResults"][0]["peerId"], "unavailable");
    assert!(rows[3]["deliveryResults"][0]["error"].is_string());
    let note_id = &rows[3]["id"];
    // Retrying the independent note reuses its existing outbox ID and
    // doesn't re-forward the already successful original.
    let _ = execute(
        &server,
        format!("mutation {{ retryChatItem(id:{note_id}) {{id}} }}"),
    )
    .await;
    assert_eq!(state.db.get_all_chats().len(), 5);
    assert_eq!(
        plain_rs::infrastructure::db::chat_store::messages::get(
            &state.db,
            rows[0]["id"].as_str().unwrap()
        )
        .unwrap()
        .unwrap()
        .status,
        plain_rs::features::chat::enums::ChatStatus::Sent
    );

    let blank = execute(&server, operation("   ")).await;
    assert_eq!(blank["forwardChatItems"].as_array().unwrap().len(), 2);

    state
        .db
        .with_conn(|db| {
            db.execute_batch(
                "CREATE TRIGGER fixture_abort_delivery BEFORE UPDATE ON chats
         WHEN NEW.to_id='unavailable' AND instr(NEW.content,'outbox note')>0
         BEGIN SELECT RAISE(ABORT,'fixture delivery failure'); END;",
            )
        })
        .unwrap();
    let committed = execute(&server, operation("outbox note")).await;
    let rows = committed["forwardChatItems"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        4,
        "after commit, return the durable outbox records"
    );
    assert_eq!(rows[0]["status"], "SENT");
    assert_eq!(rows[1]["status"], "SENT");
    assert_eq!(rows[2]["status"], "FAILED");
    assert_eq!(
        rows[3]["status"], "PENDING",
        "don't misreport an ambiguous delivery"
    );
    assert_eq!(state.db.get_all_chats().len(), 11);
    server.shutdown().await;
}

#[tokio::test]
async fn forwarding_delivers_the_original_then_the_note_in_two_peer_messages() {
    use plain_rs::features::chat::enums::{DeviceType, PeerStatus};
    use plain_rs::features::services::peers::ble_transport;
    use plain_rs::infrastructure::db::{
        DPeer,
        chat_store::{SaveMode, peers},
    };

    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = server(dir.path());
    let state = server.runtime_state();
    let (signing, public) = plain_libs::crypto::ed25519_generate();
    let base64 = plain_libs::utils::base64::base64_encode;
    state
        .prefs
        .set("signature_private_key", base64(&signing[..32]))
        .unwrap();
    let mut peer = DPeer::new("peer", "Recipient", "", Some(443), DeviceType::Phone);
    let key = [7; 32];
    peer.status = PeerStatus::Paired;
    peer.public_key = base64(&public);
    peer.key = base64(&key);
    peers::save(&state.db, &[peer], SaveMode::Insert).unwrap();
    let original = plain_rs::features::chat::messages::message_lifecycle::create(
        &state.db,
        "local",
        "",
        r#"{"type":"MESSAGE","value":{"format":"MARKDOWN","items":[],"text":"**original**"}}"#,
    )
    .unwrap();

    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let (host, ble) = (state.host.clone(), state.ble_transport.clone());
    let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let wire = ble_transport::test_peer_wire(&ble, &request["params"], &key);
                    let message: Value = plain_libs::serde_json::from_str(&wire).unwrap();
                    assert_eq!(
                        message["query"],
                        plain_rs::infrastructure::transport::contract::CREATE_CHAT_ITEM
                    );
                    let content: Value = plain_libs::serde_json::from_str(
                        message["variables"]["content"].as_str().unwrap(),
                    )
                    .unwrap();
                    sent.send(content).unwrap();
                    ble_transport::test_reply(
                        &ble,
                        &request["params"],
                        json!({"data":{"createChatItem":[]}}),
                        &key,
                    )
                }
                _ => Value::Null,
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    let result = execute(&server, format!(
        r#"mutation {{ forwardChatItems(id:{},targets:["peer:peer"],note:"attached note") {{ id status }} }}"#,
        json!(original.id),
    )).await;
    let rows = result["forwardChatItems"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row["status"] == "SENT"));
    assert_eq!(
        received.try_recv().unwrap()["value"]["text"],
        "**original**"
    );
    let note = received.try_recv().unwrap();
    assert_eq!(
        note,
        json!({"type":"MESSAGE","value":{"text":"attached note","format":"PLAIN","items":[]}})
    );
    assert!(received.try_recv().is_err());
    server.shutdown().await;
    responder.abort();
}
