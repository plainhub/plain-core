use super::*;
use plain_libs::protocols::http_client;
use plain_libs::serde_json::Value;
use plain_libs::serde_json::json;
#[tokio::test]
async fn peer_schema_preserves_required_arguments_and_exposes_channel_state() {
    let schema = schema();
    let sdl = schema.sdl();
    for field in [
        "createChatItem(content: String!): [ChatItem!]!",
        "channelState(channelId: ID!): ChannelState",
        "notifyChannelInvite(channelId: ID!, input: ChannelStateInput!): Boolean!",
        "notifyChannelStateChanged(channelId: ID!, input: ChannelStateInput!): Boolean!",
        "notifyChannelMemberRemoved(channelId: ID!, removedPeerId: ID!, version: Long!): Boolean!",
        "notifyChannelDeleted(channelId: ID!, version: Long!): Boolean!",
        "notifyChannelInviteResponse(channelId: ID!, accept: Boolean!): Boolean!",
        "notifyChannelMemberLeft(channelId: ID!): Boolean!",
        "deviceType: DeviceType",
        "members: [ChannelMemberInput!]!",
        "createdAt: Instant!",
        "updatedAt: Instant!",
        "fromId: ID!",
        "channelId: ID",
    ] {
        assert!(sdl.contains(field), "{field}: {sdl}");
    }
    assert!(!sdl.contains("_peerSchemaVersion"));
    assert!(!sdl.contains("startAware"));
    assert!(!sdl.contains("channelSystemMessage"));
    assert!(!sdl.contains("ChannelReceipt"));
    for operation in [
        "mutation { startAware }",
        r#"mutation { setPref(key:"wifi_aware_enabled", value:true) }"#,
    ] {
        assert!(!schema.execute(operation).await.errors.is_empty());
    }
    let result = schema.execute("mutation { createChatItem { id } }").await;
    assert!(!result.errors.is_empty());
    let result = schema
        .execute("{ __type(name: \"Query\") { fields { name } } }")
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.data.into_json().unwrap()["__type"]["fields"],
        json!([{"name":"channelState"}])
    );
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn encrypted_public_and_ble_rpc_share_auth_executor_and_receipts() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    use crate::server::ContentServer;
    use plain_libs::tokio_tungstenite::tungstenite::Message;
    use plain_rs::features::chat::enums::DeviceType;
    use plain_rs::features::chat::enums::PeerStatus;
    use plain_rs::infrastructure::db::DPeer;
    use plain_rs::infrastructure::db::Db;
    use plain_rs::infrastructure::db::chat_store::SaveMode;
    use plain_rs::infrastructure::db::chat_store::messages;
    use plain_rs::infrastructure::db::chat_store::peers;
    use std::sync::Arc;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("device_name", "fixture tablet").unwrap();
    prefs
        .set("client_id", "706c6169-0000-8000-8000-000000000001")
        .unwrap();
    let (local_keypair, local_public_key) = plain_libs::crypto::ed25519_generate();
    prefs
        .set(
            "signature_private_key",
            plain_libs::utils::base64::base64_encode(&local_keypair[..32]),
        )
        .unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[5; 32]);
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let key = [7; 32];
    let (kp, public_key) = plain_libs::crypto::ed25519_generate();
    let mut peer = DPeer::new(
        "706c6169-89f1-81c6-8bc7-6b67c2ffa178",
        "phone",
        "",
        Some(443),
        DeviceType::Phone,
    );
    peer.status = PeerStatus::Paired;
    peer.key = plain_libs::utils::base64::base64_encode(&key);
    peer.public_key = plain_libs::utils::base64::base64_encode(&public_key);
    peer.encryption_public_key = Some(plain_libs::crypto::hpke::public_key(&[22;32]));
    peers::save(&db, &[peer], SaveMode::Insert).unwrap();
    let server = ContentServer::start(dir.path(), &token, prefs.clone()).unwrap();
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, https) = server.start_public(0, 0, cert_pem, cert_key).await.unwrap();
    let mut host = crate::host::events::TestHost::new(&server);
    let (broadcast_done, broadcast_received) = tokio::sync::oneshot::channel();
    let ble = server.runtime_state().ble_transport.clone();
    let client_id = server.runtime_state().native_client_id().unwrap();
    let advertised_client_id = client_id.clone();
    let responder = tokio::spawn(async move {
        let mut broadcast_done = Some(broadcast_done);
        let mut count = 0;
        while let Some(Ok(message)) = host.next().await {
            if !message.is_text() {
                continue;
            }
            let request: Value =
                plain_libs::serde_json::from_str(message.to_text().unwrap()).unwrap();
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["WIFI_AWARE"]),
                "mdnsMulticast" => {
                    assert!(request["params"]["acquire"].is_boolean());
                    assert!(!request["params"]["lease"].as_str().unwrap().is_empty());
                    json!(true)
                }
                "peerStartAware" => {
                    let port = if count == 0 { 443 } else { 5443 };
                    assert_eq!(
                        request["params"],
                        json!({
                            "peer":{"id":"706c6169-89f1-81c6-8bc7-6b67c2ffa178","port":port},
                            "config":{"pmk":plain_libs::utils::base64::base64_encode(&[7;32]),"isClient":advertised_client_id.as_str() < "706c6169-89f1-81c6-8bc7-6b67c2ffa178","localPort":https}
                        })
                    );

                    let result = json!(count == 1);
                    count += 1;
                    result
                }
                "peerStopAware" => json!(true),
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let plain_rs::infrastructure::protocols::ble::Request::PeerGraphql { channel_id, body, .. } =
                        ble.request_for_test(&request["params"]) else { panic!("Expected peer request") };
                    let (wire, response_key) = plain_rs::features::services::peers::ble_transport::test_channel_wire(
                        &body, &[22;32], &channel_id,
                        "706c6169-89f1-81c6-8bc7-6b67c2ffa178",
                        &plain_libs::utils::base64::base64_encode(&local_public_key),
                    );
                    let parts: plain_rs::infrastructure::transport::graphql_request::SignedEnvelope = plain_libs::serde_json::from_str(&wire).unwrap();
                    let document: Value = plain_libs::serde_json::from_str(&wire).unwrap();
                    assert!(parts.query.contains("notifyChannelStateChanged"));
                    let local = document["variables"]["input"]["members"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|p| p["peerId"] == advertised_client_id)
                        .unwrap();
                    assert!(
                        plain_rs::features::client_identity::parse(
                            local["peerId"].as_str().unwrap()
                        )
                        .is_some()
                    );
                    assert_eq!(local["name"], "fixture tablet");
                    assert_eq!(local["deviceType"], "COMPUTER");
                    if let Some(done) = broadcast_done.take() {
                        let _ = done.send(());
                    }
                    plain_rs::features::services::peers::ble_transport::test_reply(
                        &ble,
                        &request["params"],
                        json!({"data":{"notifyChannelStateChanged":true}}),
                        &response_key,
                    )
                }
                _ => panic!("Unexpected Host call: HTTPS must bypass httpExchange: {request}"),
            };
            host.send(Message::Text(
                json!({"id":request["id"],"result":result})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
        }
    });
    let client = http_client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .unwrap();
    let wire = |document: Value, message_id: Option<&str>| {
        use plain_rs::infrastructure::transport::contract::{PeerDispatch, peer_request};
        let target = DPeer::new(&client_id, "Local", "", None, DeviceType::Computer);
        let dispatch = PeerDispatch {
            peer: &target, client_id: "706c6169-89f1-81c6-8bc7-6b67c2ffa178", channel_id: "",
            key: &key, signing_key: &kp, message_id,
        };
        let envelope = peer_request(
            document["query"].as_str().unwrap(),
            document.get("variables").cloned().unwrap_or_else(|| json!({})),
        ).unwrap();
        plain_rs::features::services::peers::ble_transport::test_seal_peer_request(&dispatch, &envelope).0
    };
    let mut received_events = server.runtime_state().events.subscribe();
    let public_url = format!("https://127.0.0.1:{https}/peer_graphql");
    let body = wire(
        json!({"query":"mutation Incoming($content:String!) { renamed:createChatItem(content:$content) { ...Fields } } fragment Fields on ChatItem { id fromId channelId content { ... on ChatMessageContent { __typename text format items { type attachment { id uri fileName size width height durationMs summary } } } ... on ChatShareContent { __typename shareId urlToken peerInfo { id ip port } name itemCount totalSize expiresAt } } createdAt updatedAt status }", "operationName":"Incoming", "variables":{"content":json!({"type":"MESSAGE","value":{"format":"PLAIN","items":[],"text":"hello"}}).to_string()}}),
        Some("fixture-message"),
    );
    let response = client
        .post(&public_url)
        .header("c-id", "706c6169-89f1-81c6-8bc7-6b67c2ffa178")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let content = plain_rs::infrastructure::transport::http_response::Context::new(&body)
        .decrypt(&key, &response.bytes().await.unwrap())
        .unwrap();
    let content: Value = plain_libs::serde_json::from_slice(&content).unwrap();
    assert!(content.get("errors").is_none(), "{content}");
    let id = content["data"]["renamed"][0]["id"].as_str().unwrap();
    assert_eq!(
        messages::get(&db, id).unwrap().unwrap().from_id,
        "706c6169-89f1-81c6-8bc7-6b67c2ffa178"
    );
    assert!(content["data"]["renamed"][0]["channelId"].is_null());
    let event = std::iter::from_fn(|| received_events.try_recv().ok())
        .take(32)
        .find(|event| event.event_type == plain_rs::features::chat::events::WS_MESSAGE_CREATED)
        .expect("received message event");
    let stored = messages::get(&db, id).unwrap().unwrap();
    assert_eq!(
        plain_libs::serde_json::from_str::<Value>(&event.payload).unwrap(),
        json!([plain_rs::features::chat::service::chat_to_json(&stored)])
    );
    let body = wire(json!({"query":"mutation { startAware }"}), None);
    let response = client
        .post(&public_url)
        .header("c-id", "706c6169-89f1-81c6-8bc7-6b67c2ffa178")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    let content: Value = plain_libs::serde_json::from_slice(
        &plain_rs::infrastructure::transport::http_response::Context::new(&body)
            .decrypt(&key, &response.bytes().await.unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(
        content["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty())
    );
    let body = wire(
        json!({"query":"mutation { notifyChannelDeleted(channelId:\"unknown\",version:-1) }"}), None,
    );
    let response = client
        .post(&public_url)
        .header("c-id", "706c6169-89f1-81c6-8bc7-6b67c2ffa178")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    let content: Value = plain_libs::serde_json::from_slice(
        &plain_rs::infrastructure::transport::http_response::Context::new(&body)
            .decrypt(&key, &response.bytes().await.unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(content["data"].is_null());
    assert_eq!(content["errors"][0]["message"], "Invalid channel version");
    assert_eq!(
        client
            .post(&public_url)
            .header("c-id", "706c6169-443e-8fb9-853c-6a0a69b6663d")
            .body(wire(json!({"query":"mutation { startAware }"}), None))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let channel =
        plain_rs::features::chat::channels::state::create(&db, &client_id, "fixture").unwrap();
    plain_rs::features::chat::channels::state::apply(
        &db,
        &client_id,
        &channel.id,
        plain_rs::features::chat::channels::state::Action::Invite {
            peer: "706c6169-89f1-81c6-8bc7-6b67c2ffa178".into(),
        },
    )
    .unwrap();
    let mut channel_events = server.runtime_state().events.subscribe();
    let body = wire(
        json!({"query":"mutation Accept($channelId:ID!) { notifyChannelInviteResponse(channelId:$channelId, accept:true) }","variables":{"channelId":channel.id}}), None,
    );
    let response = client
        .post(&public_url)
        .header("c-id", "706c6169-89f1-81c6-8bc7-6b67c2ffa178")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    let content: Value = plain_libs::serde_json::from_slice(
        &plain_rs::infrastructure::transport::http_response::Context::new(&body)
            .decrypt(&key, &response.bytes().await.unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        content["data"]["notifyChannelInviteResponse"], true,
        "{content}"
    );
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Ok(event) = channel_events.recv().await {
            if event.event_type == plain_rs::features::chat::events::WS_CHANNELS_UPDATED {
                return event;
            }
        }
        panic!("Channel event stream closed");
    })
    .await
    .unwrap();
    let event: Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
    assert_eq!(event["channels"][0]["id"], channel.id);
    assert_eq!(event["channels"][0]["members"][1]["status"], "JOINED");
    assert!(event["channels"][0].get("key").is_none());

    tokio::time::timeout(std::time::Duration::from_secs(2), broadcast_received)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client
            .post(&public_url)
            .header("c-id", "706c6169-89f1-81c6-8bc7-6b67c2ffa178")
            .body(wire(json!({"query":"mutation { startAware }"}), None))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    server.shutdown().await;
    responder.abort();
}

#[tokio::test]
async fn signed_channel_notifications_apply_once_and_enforce_route_and_recipient() {
    use plain_libs::utils::base64::base64_encode;
    use plain_rs::features::chat::enums::{ChannelStatus, DeviceType, PeerStatus};
    use plain_rs::infrastructure::db::DPeer;
    use plain_rs::infrastructure::db::chat_store::{SaveMode, channels, peers};
    use plain_rs::infrastructure::transport::{
        contract::{PeerDispatch, peer_request}, http_response,
    };
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap(),
    );
    let server =
        crate::server::ContentServer::start(dir.path(), &base64_encode(&[4; 32]), prefs).unwrap();
    let state = server.runtime_state();
    let actor = state.native_client_id().unwrap();
    let (signer, public) = plain_libs::crypto::ed25519_generate();
    let mut owner = DPeer::new("owner", "Owner", "", None, DeviceType::Computer);
    owner.key = base64_encode(&[5; 32]);
    owner.public_key = base64_encode(&public);
    owner.status = PeerStatus::Paired;
    owner.encryption_public_key = Some(plain_libs::crypto::hpke::public_key(&[7;32]));
    peers::save(&state.db, &[owner], SaveMode::Insert).unwrap();
    let local_seed = state.prefs.get::<String>("encryption_private_key").unwrap().unwrap();
    let local_seed: [u8;32] = plain_libs::utils::base64::base64_decode(&local_seed).try_into().unwrap();
    let local_key = plain_libs::crypto::hpke::public_key(&local_seed);
    let local_signature = plain_rs::features::prefs::security::get_entry(&state.prefs, "signature_public_key")
        .unwrap().unwrap();
    let target = DPeer::new(&actor, "Local", "", None, DeviceType::Phone);
    let request = |query: &'static str, variables: Value, context: &'static str, key: [u8; 32]| {
        let state = &state;
        let signer = &signer;
        let target = &target;
        async move {
            let wire = peer_request(query, variables).unwrap();
            let key = if context.is_empty() { key } else { local_key };
            let dispatch = PeerDispatch { peer: target, client_id: "owner", channel_id: context,
                key: &key, signing_key: signer, message_id: None };
            let (body, key) = plain_rs::features::services::peers::ble_transport::test_seal_peer_request(&dispatch, &wire);
            if context.is_empty() {
                let plaintext = plain_rs::infrastructure::transport::http_request::decrypt_raw(
                    &key, "POST", "/peer_graphql", &body,
                ).unwrap();
                assert!(plain_libs::serde_json::from_slice::<Value>(&plaintext).unwrap()["signature"].as_str().is_some_and(|signature| !signature.is_empty()));
                assert_eq!(body.len(), 24 + plaintext.len() + 16);
            }
            let (status, response) = execute(state, "owner", context, &body).await;
            assert_eq!(status, 200);
            assert_eq!(execute(state, "owner", context, &body).await.0, 400);
            let data = http_response::Context::new(&body)
                .decrypt(&key, &response)
                .unwrap();
            plain_libs::serde_json::from_slice::<Value>(&data).unwrap()
        }
    };
    let invitation = "mutation($channelId:ID!,$input:ChannelStateInput!){notifyChannelInvite(channelId:$channelId,input:$input)}";
    let sign = |snapshot: &mut Value| {
        let input = json!(["channel-roster", "channel", snapshot["version"], snapshot["name"], snapshot["members"]]);
        snapshot["signature"] = json!(plain_libs::crypto::ed25519_sign(&signer, input.to_string().as_bytes()));
    };
    let (member_signer, member_signature) = plain_libs::crypto::ed25519_generate();
    let mut snapshot = json!({"name":"Channel","version":1,"members":[
        {"peerId":"owner","status":"JOINED","deviceType":"COMPUTER","signaturePublicKey":base64_encode(&public),
            "encryptionPublicKey":base64_encode(&plain_libs::crypto::hpke::public_key(&[7;32]))},
        {"peerId":actor,"status":"PENDING","signaturePublicKey":local_signature,"encryptionPublicKey":base64_encode(&local_key)},
        {"peerId":"new-peer","status":"JOINED","deviceType":"PHONE","port":443,"signaturePublicKey":base64_encode(&member_signature),"encryptionPublicKey":base64_encode(&[83;32])}
    ]});
    sign(&mut snapshot);
    let variables = json!({"channelId":"channel","input":snapshot});
    for _ in 0..2 {
        let accepted = request(invitation, variables.clone(), "", [5; 32]).await;
        assert_eq!(accepted["data"]["notifyChannelInvite"], true, "{accepted}");
    }
    assert_eq!(
        peers::get(&state.db, "new-peer").unwrap().unwrap().port,
        Some(443)
    );
    let blocked = request(invitation, variables, "channel", [7; 32]).await;
    assert_eq!(
        blocked["errors"][0]["message"],
        "Channel invitations require direct pairing"
    );
    let mismatch = request(
        "mutation{notifyChannelDeleted(channelId:\"different\",version:7)}",
        json!({}),
        "channel",
        [7; 32],
    )
    .await;
    assert_eq!(
        mismatch["errors"][0]["message"],
        "Channel transport context mismatch"
    );
    let mut update = snapshot;
    update["version"] = json!(2);
    update["name"] = json!("Renamed");
    update["members"][1]["status"] = json!("JOINED");
    sign(&mut update);
    let reply = request("mutation($input:ChannelStateInput!){notifyChannelStateChanged(channelId:\"channel\",input:$input)}", json!({"input":update}), "channel", [7;32]).await;
    assert_eq!(reply["data"]["notifyChannelStateChanged"], true, "{reply}");
    assert_eq!(
        channels::get(&state.db, "channel").unwrap().unwrap().name,
        "Renamed"
    );
    let dispatch = PeerDispatch { peer: &target, client_id: "owner", channel_id: "channel",
        key: &local_key, signing_key: &signer, message_id: Some("chat-message") };
    for query in [
        "mutation($content:String!){ createChatItem(content:$content){id} createChatItem(content:$content){id} }",
        "{ channelState(channelId:\"channel\") { version } }",
        "mutation { notifyChannelDeleted(channelId:\"channel\",version:3) }",
        r#"mutation($content: String!) { createChatItem(content:$content) { id } ...Control }
           fragment Control on Mutation { notifyChannelDeleted(channelId:"channel",version:3) }"#,
    ] {
        let control = peer_request(
            query,
            json!({"content":r#"{"type":"MESSAGE","value":{"text":"fixture","format":"PLAIN","items":[]}}"#}),
        ).unwrap();
        let (body, _) = plain_rs::features::services::peers::ble_transport::test_seal_peer_request(&dispatch, &control);
        assert_eq!(execute(&state, "owner", "channel", &body).await.0, 400);
    }
    assert!(state.db.get_all_chats().is_empty());
    assert_eq!(channels::get(&state.db, "channel").unwrap().unwrap().status, ChannelStatus::Joined);
    let send_message = |id: &'static str| {
        let state = &state;
        let target = &target;
        let member_signer = &member_signer;
        async move {
            let wire = plain_rs::infrastructure::transport::contract::chat_item_request(
                r#"{"type":"MESSAGE","value":{"text":"unpaired delivery","format":"PLAIN","items":[]}}"#,
            ).unwrap();
            let dispatch = PeerDispatch { peer: target, client_id: "new-peer", channel_id: "channel",
                key: &local_key, signing_key: member_signer, message_id: Some(id) };
            let (body, key) = plain_rs::features::services::peers::ble_transport::test_seal_peer_request(&dispatch, &wire);
            let local_peer = peers::get(&state.db, "new-peer").unwrap().unwrap();
            let (plaintext, _) = plain_rs::features::services::peers::ble_transport::test_channel_wire(
                &body, &local_seed, "channel", &target.id, &local_peer.public_key,
            );
            assert_eq!(body.len(), 32 + plaintext.len() + 16);
            let payload: Value = plain_libs::serde_json::from_str(&plaintext).unwrap();
            assert_eq!(payload["messageId"], id);
            assert!(!payload["signature"].as_str().unwrap().is_empty());
            let (status, response) = execute(state, "new-peer", "channel", &body).await;
            assert_eq!(status, 200);
            let data = http_response::Context::new(&body).decrypt(&key, &response).unwrap();
            plain_libs::serde_json::from_slice::<Value>(&data).unwrap()
        }
    };
    let first = send_message("message-id").await;
    assert_eq!(first["data"]["createChatItem"][0]["fromId"], "new-peer", "{first}");
    let retry = send_message("message-id").await;
    assert_eq!(retry["data"]["createChatItem"], json!([]), "{retry}");
    assert_eq!(state.db.get_all_chats().len(), 1);

    let removal = "mutation($peerId:ID!){notifyChannelMemberRemoved(channelId:\"channel\",removedPeerId:$peerId,version:3)}";
    let wrong = request(removal, json!({"peerId":"different"}), "", [5; 32]).await;
    assert_eq!(
        wrong["errors"][0]["message"],
        "Channel removal recipient mismatch"
    );
    assert_eq!(
        channels::get(&state.db, "channel")
            .unwrap()
            .unwrap()
            .version,
        2
    );
    let right = request(removal, json!({"peerId":actor}), "", [5; 32]).await;
    assert_eq!(right["data"]["notifyChannelMemberRemoved"], true, "{right}");
    let row = channels::get(&state.db, "channel").unwrap().unwrap();
    assert_eq!(row.status, ChannelStatus::Kicked);
    assert_eq!(row.version, 3);
    let after_removal = send_message("fresh-message").await;
    assert_eq!(after_removal["errors"][0]["message"], "Channel member not joined", "{after_removal}");
    assert_eq!(state.db.get_all_chats().len(), 1);
    server.shutdown().await;
}

#[test]
fn peer_schema_matches_committed_sdl() {
    let actual = schema().sdl();
    if std::env::var_os("UPDATE_GRAPHQL_SCHEMAS").is_some() {
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/peer-api.graphqls"),
            &actual,
        )
        .unwrap();
    } else {
        assert_eq!(
            actual,
            include_str!("../../../../../../schema/peer-api.graphqls")
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delivery_retries_after_lost_ack_and_restart_only_store_one_message() {
    use plain_libs::utils::base64::base64_encode;
    use plain_rs::features::chat::channels::{messages::ChannelMember, state as channel_state};
    use plain_rs::features::chat::enums::{DeviceType, PeerStatus};
    use plain_rs::infrastructure::db::{DPeer, chat_store::{SaveMode, channels, peers}};
    use plain_rs::infrastructure::transport::{contract::{PeerDispatch, chat_item_request}, http_response};
    use plain_rs::features::services::peers::ble_transport::test_seal_peer_request;
    use crate::server::ContentServer;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs_path = dir.path().join("prefs.json");
    let token = base64_encode(&[4; 32]);
    let start = || {
        let prefs = std::sync::Arc::new(plain_rs::features::prefs::Prefs::load(&prefs_path).unwrap());
        ContentServer::start(dir.path(), &token, prefs).unwrap()
    };
    let server = start();
    let state = server.runtime_state();
    let actor = state.native_client_id().unwrap();
    let target = DPeer::new(&actor, "Local", "", None, DeviceType::Computer);
    let (signer, public) = plain_libs::crypto::ed25519_generate();
    let key = [5; 32];
    let mut sender = DPeer::new("sender", "Phone", "", None, DeviceType::Phone);
    sender.status = PeerStatus::Paired;
    sender.key = base64_encode(&key);
    sender.public_key = base64_encode(&public);
    peers::save(&state.db, &[sender], SaveMode::Insert).unwrap();
    let mut channel = channel_state::create(&state.db, &actor, "Fixture").unwrap();
    channel.members = plain_libs::serde_json::to_string(&[
        ChannelMember::new(&actor), ChannelMember::new("sender"),
    ]).unwrap();
    channels::save(&state.db, std::slice::from_ref(&channel), SaveMode::Update).unwrap();
    let encryption_key = plain_rs::features::prefs::security::encryption_public_key(&state.prefs).unwrap();

    let make_request = || chat_item_request(
        r#"{"type":"MESSAGE","value":{"text":"lost receipt","format":"PLAIN","items":[]}}"#,
    ).unwrap();
    let message_id = plain_rs::features::record_id::generate();
    let modes = [("", &key), (channel.id.as_str(), &encryption_key)];
    let dispatch = |channel_id, key, message_id| PeerDispatch {
        peer: &target, client_id: "sender", channel_id, key, signing_key: &signer, message_id,
    };
    let mut first_attempts = Vec::new();
    for (channel_id, key) in modes {
        let request = make_request();
        let (missing, missing_key) = test_seal_peer_request(&dispatch(channel_id, key, None), &request);
        let (status, response) = execute(&state, "sender", channel_id, &missing).await;
        assert_eq!(status, 200);
        let data = http_response::Context::new(&missing).decrypt(&missing_key, &response).unwrap();
        let result: Value = plain_libs::serde_json::from_slice(&data).unwrap();
        assert_eq!(result["errors"][0]["message"], "Missing signed message ID");
        let (first, key) = test_seal_peer_request(&dispatch(channel_id, key, Some(&message_id)), &make_request());
        let (status, _lost_ack) = execute(&state, "sender", channel_id, &first).await;
        assert_eq!(status, 200);
        first_attempts.push((channel_id.to_owned(), first, key));
    }
    let original: std::collections::HashSet<_> = state.db.get_all_chats().into_iter().map(|chat| chat.id).collect();
    assert_eq!(original.len(), 2);
    drop(state);
    server.shutdown().await;

    let server = start();
    let state = server.runtime_state();
    assert_eq!(state.native_client_id().unwrap(), actor);
    let mut events = state.events.subscribe();
    for ((channel_id, key), first) in modes.into_iter().zip(first_attempts) {
        let (retry, response_key) = test_seal_peer_request(&dispatch(channel_id, key, Some(&message_id)), &make_request());
        let assert_duplicate = |attempt: (String, Vec<u8>, [u8; 32])| {
            let state = state.clone();
            tokio::spawn(async move {
                let (channel_id, body, key) = attempt;
                let (status, response) = execute(&state, "sender", &channel_id, &body).await;
                assert_eq!(status, 200);
                let response = http_response::Context::new(&body).decrypt(&key, &response).unwrap();
                let result: Value = plain_libs::serde_json::from_slice(&response).unwrap();
                assert_eq!(result, json!({"data":{"createChatItem":[]}}));
            })
        };
        let a = assert_duplicate(first);
        let b = assert_duplicate((channel_id.to_owned(), retry, response_key));
        a.await.unwrap();
        b.await.unwrap();
    }
    let current: std::collections::HashSet<_> = state.db.get_all_chats().into_iter().map(|chat| chat.id).collect();
    assert_eq!(current, original);
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, plain_rs::features::chat::events::WS_MESSAGE_CREATED);
    }
    server.shutdown().await;
}
