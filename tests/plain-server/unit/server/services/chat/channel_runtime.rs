use plain_libs::serde_json::Value;
use plain_libs::serde_json::json;
use plain_rs::features::chat::channels::state;
use plain_rs::features::chat::channels::state::Action;
use plain_rs::features::services::chat::channel_runtime::*;
use plain_rs::infrastructure::db::chat_store::channels;
use plain_rs::infrastructure::db::chat_store::peers;
use plain_rs::runtime::Runtime as ServerState;

#[cfg(feature = "http_transport")]
fn fixture() -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    use std::sync::Arc;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let (key, _public) = plain_libs::crypto::ed25519_generate();
    prefs.set("device_name", "Actual tablet").unwrap();
    prefs
        .set(
            "signature_private_key",
            plain_libs::utils::base64::base64_encode(&key[..32]),
        )
        .unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}
#[cfg(feature = "http_transport")]
fn add_peer(state: &ServerState, id: &str, paired: bool) {
    let mut peer = plain_rs::infrastructure::db::DPeer::new(
        id,
        id,
        "",
        Some(2443),
        plain_rs::features::chat::enums::DeviceType::Tablet,
    );
    peer.encryption_public_key = Some(plain_libs::crypto::hpke::public_key(&[22; 32]));
    peer.public_key = plain_libs::utils::base64::base64_encode(&[9; 32]);
    if paired {
        peer.key = plain_libs::utils::base64::base64_encode(&[8; 32]);
        peer.status = plain_rs::features::chat::enums::PeerStatus::Paired;
    }
    peers::save(
        &state.db,
        &[peer],
        plain_rs::infrastructure::db::chat_store::SaveMode::Insert,
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
    let channel = state::create(&state.db, &state.native_client_id().unwrap(), "fixture").unwrap();
    let (generation, mut rx) = crate::test_support::HostRequests::connect(&state.host);
    let host = state.host.clone();
    let prefs = state.prefs.clone();
    let actor = state.native_client_id().unwrap();
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
                    let plain_rs::infrastructure::protocols::ble::Request::PeerGraphql { body, channel_id: cid, .. } = ble.request_for_test(p) else { panic!("Expected peer request") };
                    let id = p["peer"]["id"].as_str().unwrap();
                    let public = plain_libs::utils::base64::base64_encode(
                        &plain_rs::features::prefs::security::signing_keypair(&prefs).unwrap()[32..],
                    );
                    let (body, key) = if cid.is_empty() {
                        let key = [8; 32];
                        let body = plain_rs::features::services::peers::ble_transport::test_peer_wire(&ble, p, &key);
                        (body, key)
                    } else {
                        plain_rs::features::services::peers::ble_transport::test_channel_wire(
                            &body, &[22; 32], &cid, &actor, id, &public,
                        )
                    };
                    let pieces: plain_rs::infrastructure::transport::graphql_request::SignedEnvelope = plain_libs::serde_json::from_str(&body).unwrap();
                    let wire: Value = plain_libs::serde_json::from_str(&body).unwrap();
                    let field = [
                        "notifyChannelInvite",
                        "notifyChannelStateChanged",
                        "notifyChannelDeleted",
                    ]
                    .into_iter()
                    .find(|field| pieces.query.contains(field))
                    .unwrap();
                    if field == "notifyChannelInvite" {
                        assert_eq!(p["peer"]["id"], "paired");
                        assert_eq!(
                            wire["variables"]["input"]["state"]["members"][0]["deviceType"],
                            "COMPUTER"
                        );
                    }
                    if field == "notifyChannelInvite" {
                        assert!(
                            matches!(ble.request_for_test(p), plain_rs::infrastructure::protocols::ble::Request::PeerGraphql { channel_id, .. } if channel_id.is_empty())
                        );
                    } else {
                        assert!(
                            matches!(ble.request_for_test(p), plain_rs::infrastructure::protocols::ble::Request::PeerGraphql { channel_id: cid, .. } if cid == channel_id)
                        );
                    }
                    attempts.push(field);
                    plain_rs::features::services::peers::ble_transport::test_reply(
                        &ble,
                        p,
                        json!({"data":{(field):attempts.len()!=1}}),
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
    let before = get(&state, &channel.id).unwrap();
    assert!(
        execute(
            &state,
            Request::Invite {
                id: channel.id.clone(),
                peer: "group".into()
            }
        )
        .await
        .is_err()
    );
    assert_eq!(get(&state, &channel.id).unwrap(), before);
    state::apply(
        &state.db,
        &state.native_client_id().unwrap(),
        &channel.id,
        Action::Invite {
            peer: "group".into(),
        },
    )
    .unwrap();
    execute(
        &state,
        Request::Rename {
            id: channel.id.clone(),
            name: "Updated".into(),
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
    assert_eq!(
        attempts
            .iter()
            .filter(|s| **s == "notifyChannelDeleted")
            .count(),
        2
    );
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
    channel.members = plain_libs::serde_json::to_string(&[
        plain_rs::features::chat::channels::messages::ChannelMember::new("owner"),
        plain_rs::features::chat::channels::messages::ChannelMember::pending(
            &state.native_client_id().unwrap(),
        ),
    ])
    .unwrap();
    channels::save(
        &state.db,
        &[channel.clone()],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
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
    state.host.reply(generation,json!({"id":attempt["id"],"result":plain_rs::features::services::peers::ble_transport::test_reply(&state.ble_transport, &attempt["params"], json!({"data":{"notifyChannelInviteResponse":true}}),&[8;32])})).unwrap();
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
    channel.members = plain_libs::serde_json::to_string(&[
        plain_rs::features::chat::channels::messages::ChannelMember::new("owner"),
        plain_rs::features::chat::channels::messages::ChannelMember::pending(
            &state.native_client_id().unwrap(),
        ),
    ])
    .unwrap();
    channels::save(
        &state.db,
        &[channel.clone()],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
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
    peer.key = plain_libs::utils::base64::base64_encode(&[9; 32]);
    peers::save(
        &state.db,
        &[peer],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
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
    let actor = state.native_client_id().unwrap();
    let sender = plain_rs::features::prefs::security::get_entry(&state.prefs, "signature_public_key")
        .unwrap().unwrap().as_str().unwrap().to_owned();
    let ble = state.ble_transport.clone();
    let responder = tokio::spawn(async move {
        let mut kinds = Vec::new();
        while kinds.len() < 3 {
            let req = rx.recv().await.unwrap();
            let result = match req["method"].as_str().unwrap() {
                "peerTransportCapabilities" => json!(["BLE"]),
                "peerTransportBleExchange" => {
                    let p = &req["params"];
                    let plain_rs::infrastructure::protocols::ble::Request::PeerGraphql { channel_id, body, .. } = ble.request_for_test(p) else { panic!("Expected peer request") };
                    let (body, response_key) = plain_rs::features::services::peers::ble_transport::test_channel_wire(
                        &body, &[22; 32], &channel_id, &actor, "owner", &sender,
                    );
                    let wire: Value = plain_libs::serde_json::from_str::<Value>(&body).unwrap();
                    let query = wire["query"].as_str().unwrap();
                    let field = if query.contains("notifyChannelInviteResponse") {
                        "notifyChannelInviteResponse"
                    } else {
                        assert!(query.contains("notifyChannelMemberLeft"));
                        "notifyChannelMemberLeft"
                    };
                    kinds.push((field, wire["variables"]["accept"].as_bool()));
                    plain_rs::features::services::peers::ble_transport::test_reply(
                        &ble,
                        &req["params"],
                        json!({"data":{(field):true}}),
                        &response_key,
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
    assert_eq!(
        accepted["response"]["data"]["notifyChannelInviteResponse"],
        true
    );
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
        plain_rs::features::chat::enums::ChannelStatus::Left
    );
    // An owner re-invitation replaces the left state, then decline removes exactly that row.
    channels::save(
        &state.db,
        &[channel.clone()],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
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
        vec![
            ("notifyChannelInviteResponse", Some(true)),
            ("notifyChannelMemberLeft", None),
            ("notifyChannelInviteResponse", Some(false))
        ]
    );
    state.host.disconnect(generation);
    server.shutdown().await;
}
