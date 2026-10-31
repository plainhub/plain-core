use plain_rs::chat::channels::state;
use plain_rs::chat::channels::state::Action;
use plain_rs::chat::enums::ChannelSystemMessageType;
use plain_rs::db::chat_store::channels;
use plain_rs::db::chat_store::peers;
use plain_rs::runtime::Runtime as ServerState;
use plain_rs::services::chat::channel_runtime::*;
use serde_json::Value;
use serde_json::json;

#[cfg(feature = "http_transport")]
fn fixture() -> (tempfile::TempDir, crate::server::ContentServer) {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let (key, _public) = plain_rs::crypto::ed25519_generate();
    prefs.set("device_name", "Actual tablet").unwrap();
    prefs.set("client_id", "actor").unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&key),
        )
        .unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}
#[cfg(feature = "http_transport")]
fn add_peer(state: &ServerState, id: &str, paired: bool) {
    let mut peer = plain_rs::db::DPeer::new(
        id,
        id,
        "",
        Some(2443),
        plain_rs::chat::enums::DeviceType::Tablet,
    );
    if paired {
        peer.key = plain_rs::utils::base64::base64_encode(&[8; 32]);
    }
    peers::save(
        &state.db,
        &[peer],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn signed_invites_ack_keys_and_deleted_broadcast_use_rust_snapshot() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    add_peer(&state, "paired", true);
    add_peer(&state, "group", false);
    let channel = state::create(&state.db, "actor", "fixture").unwrap();
    let (generation, mut rx) = crate::test_support::HostRequests::connect(&state.host);
    let host = state.host.clone();
    let prefs = state.prefs.clone();
    let channel_key = channel.key.clone();
    let channel_id = channel.id.clone();
    let ble = state.ble_transport.clone();
    let responder = tokio::spawn(async move {
        let mut attempts = Vec::new();
        while attempts.len() < 5 {
            let req = rx.recv().await.unwrap();
            let result = match req["method"].as_str().unwrap() {
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let p = &req["params"];
                    let key = if p["peer"]["id"] == "paired" {
                        vec![8; 32]
                    } else {
                        plain_rs::utils::base64::base64_decode(&channel_key)
                    };
                    let body =
                        plain_rs::services::peers::ble_transport::test_peer_wire(&ble, p, &key);
                    let pieces: Vec<_> = body.splitn(3, '|').collect();
                    let public = plain_rs::utils::base64::base64_encode(
                        &plain_rs::services::peers::peer_wire::signing_keypair(&prefs).unwrap()
                            [32..],
                    );
                    assert!(plain_rs::crypto::ed25519_verify(
                        &public,
                        plain_rs::transport::auth::signature_payload(
                            pieces[1].parse().unwrap(),
                            if p["peer"]["id"] == "paired" {
                                ""
                            } else {
                                &channel_id
                            },
                            pieces[2]
                        )
                        .as_bytes(),
                        pieces[0]
                    ));
                    let wire: Value = serde_json::from_str(pieces[2]).unwrap();
                    let payload: Value =
                        serde_json::from_str(wire["variables"]["payload"].as_str().unwrap())
                            .unwrap();
                    if wire["variables"]["type"] == "INVITE" {
                        assert_eq!(payload["memberPeers"][0]["deviceType"], "COMPUTER");
                    }
                    if p["peer"]["id"] == "paired" {
                        assert!(
                            matches!(ble.request_for_test(p), plain_rs::protocols::ble::Request::PeerGraphql { channel_id, .. } if channel_id.is_empty())
                        );
                    } else {
                        assert!(
                            matches!(ble.request_for_test(p), plain_rs::protocols::ble::Request::PeerGraphql { channel_id: cid, .. } if cid == channel_id)
                        );
                    }
                    attempts.push(wire["variables"]["type"].as_str().unwrap().to_owned());
                    plain_rs::services::peers::ble_transport::test_reply(
                        &ble,
                        p,
                        json!({"data":{"channelSystemMessage":attempts.len()!=1}}),
                        &key,
                    )
                }
                method => panic!("Unexpected {method}"),
            };
            host.reply(generation, json!({"id":req["id"],"result":result}))
                .unwrap();
        }
        attempts
    });
    let first = execute(
        &state,
        Request::Invite {
            id: channel.id.clone(),
            peer: "paired".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        first["response"]["errors"][0]["message"],
        "Channel message rejected"
    );
    execute(
        &state,
        Request::Invite {
            id: channel.id.clone(),
            peer: "group".into(),
        },
    )
    .await
    .unwrap();
    execute(
        &state,
        Request::Send {
            id: channel.id.clone(),
            message_type: ChannelSystemMessageType::Update,
            target: "paired".into(),
        },
    )
    .await
    .unwrap();
    execute(
        &state,
        Request::Delete {
            id: channel.id.clone(),
        },
    )
    .await
    .unwrap();
    let attempts = responder.await.unwrap();
    assert_eq!(attempts.iter().filter(|s| s.as_str() == "KICK").count(), 2);
    assert!(channels::get(&state.db, &channel.id).unwrap().is_none());
    state.host.disconnect(generation);
    server.shutdown().await;
}
#[cfg(feature = "http_transport")]
#[tokio::test]
async fn queued_send_and_decline_ack_cannot_act_on_replacement() {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    add_peer(&state, "owner", true);
    let mut channel = state::create(&state.db, "owner", "invitation").unwrap();
    channel.members = serde_json::to_string(&[
        plain_rs::chat::channels::messages::ChannelMember::new("owner"),
        plain_rs::chat::channels::messages::ChannelMember::pending("actor"),
    ])
    .unwrap();
    channels::save(
        &state.db,
        &[channel.clone()],
        plain_rs::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    let (generation, mut rx) = crate::test_support::HostRequests::connect(&state.host);
    let call = {
        let state = state.clone();
        let id = channel.id.clone();
        tokio::spawn(async move { execute(&state, Request::Accept { id }).await })
    };
    let capability = rx.recv().await.unwrap();
    assert_eq!(capability["method"], "peerTransportCapabilities");
    let newer = state::apply(
        &state.db,
        "owner",
        &channel.id,
        Action::Rename {
            name: "new invitation".into(),
        },
    )
    .unwrap();
    state
        .host
        .reply(generation, json!({"id":capability["id"],"result":["BLE"]}))
        .unwrap();
    let response = call.await.unwrap().unwrap();
    assert_eq!(
        response["response"]["errors"][0]["message"],
        "Channel changed before send"
    );
    let call = {
        let state = state.clone();
        let id = channel.id.clone();
        tokio::spawn(async move { execute(&state, Request::Decline { id }).await })
    };
    let capabilities = rx.recv().await.unwrap();
    state
        .host
        .reply(
            generation,
            json!({"id":capabilities["id"],"result":["BLE"]}),
        )
        .unwrap();
    let attempt = rx.recv().await.unwrap();
    let replacement = state::apply(
        &state.db,
        "owner",
        &channel.id,
        Action::Rename {
            name: "replacement".into(),
        },
    )
    .unwrap();
    state.host.reply(generation,json!({"id":attempt["id"],"result":plain_rs::services::peers::ble_transport::test_reply(&state.ble_transport, &attempt["params"], json!({"data":{"channelSystemMessage":true}}),&[8;32])})).unwrap();
    assert!(
        call.await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("Channel changed")
    );
    assert_eq!(get(&state, &channel.id).unwrap(), replacement);
    assert_ne!(newer, replacement);
    state.host.disconnect(generation);
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn peer_key_changes_before_physical_send_are_rejected_and_accept_leave_decline_are_root_owned()
 {
    let (_dir, server) = fixture();
    let state = server.runtime_state();
    add_peer(&state, "owner", true);
    let mut channel = state::create(&state.db, "owner", "fixture").unwrap();
    channel.members = serde_json::to_string(&[
        plain_rs::chat::channels::messages::ChannelMember::new("owner"),
        plain_rs::chat::channels::messages::ChannelMember::pending("actor"),
    ])
    .unwrap();
    channels::save(
        &state.db,
        &[channel.clone()],
        plain_rs::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    let (generation, mut rx) = crate::test_support::HostRequests::connect(&state.host);
    let call = {
        let state = state.clone();
        let id = channel.id.clone();
        tokio::spawn(async move { execute(&state, Request::Accept { id }).await })
    };
    let cap = rx.recv().await.unwrap();
    let mut peer = peers::get(&state.db, "owner").unwrap().unwrap();
    peer.key = plain_rs::utils::base64::base64_encode(&[9; 32]);
    peers::save(
        &state.db,
        &[peer],
        plain_rs::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    state
        .host
        .reply(generation, json!({"id":cap["id"],"result":["BLE"]}))
        .unwrap();
    assert_eq!(
        call.await.unwrap().unwrap()["response"]["errors"][0]["message"],
        "Peer changed before send"
    );
    let host = state.host.clone();
    let ble = state.ble_transport.clone();
    let responder = tokio::spawn(async move {
        let mut kinds = Vec::new();
        while kinds.len() < 3 {
            let req = rx.recv().await.unwrap();
            let result = match req["method"].as_str().unwrap() {
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let body = plain_rs::services::peers::ble_transport::test_peer_wire(
                        &ble,
                        &req["params"],
                        &[9; 32],
                    );
                    let wire: Value =
                        serde_json::from_str(body.splitn(3, '|').nth(2).unwrap()).unwrap();
                    let kind = wire["variables"]["type"].as_str().unwrap().to_string();
                    if kind == "INVITE_ACCEPT" {
                        let payload: Value =
                            serde_json::from_str(wire["variables"]["payload"].as_str().unwrap())
                                .unwrap();
                        assert_eq!(payload["deviceType"], "COMPUTER");
                        assert_eq!(payload["name"], "Actual tablet");
                    }
                    kinds.push(kind);
                    plain_rs::services::peers::ble_transport::test_reply(
                        &ble,
                        &req["params"],
                        json!({"data":{"channelSystemMessage":true}}),
                        &[9; 32],
                    )
                }
                method => panic!("Unexpected {method}"),
            };
            host.reply(generation, json!({"id":req["id"],"result":result}))
                .unwrap();
        }
        kinds
    });
    let accepted = execute(
        &state,
        Request::Accept {
            id: channel.id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(accepted["response"]["data"]["channelSystemMessage"], true);
    assert!(
        get(&state, &channel.id)
            .unwrap()
            .members
            .contains("PENDING")
    );
    execute(
        &state,
        Request::Leave {
            id: channel.id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        get(&state, &channel.id).unwrap().status,
        plain_rs::chat::enums::ChannelStatus::Left
    );
    // An owner re-invitation replaces the left state, then decline removes exactly that row.
    channels::save(
        &state.db,
        &[channel.clone()],
        plain_rs::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    execute(
        &state,
        Request::Decline {
            id: channel.id.clone(),
        },
    )
    .await
    .unwrap();
    assert!(channels::get(&state.db, &channel.id).unwrap().is_none());
    assert_eq!(
        responder.await.unwrap(),
        vec!["INVITE_ACCEPT", "LEAVE", "INVITE_DECLINE"]
    );
    state.host.disconnect(generation);
    server.shutdown().await;
}
