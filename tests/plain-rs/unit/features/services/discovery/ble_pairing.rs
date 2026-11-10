use super::*;
use std::sync::Arc;
fn fixture(id: &str) -> (plain_libs::temporary::TempDir, crate::test_runtime::Core) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let (key, _public) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_libs::utils::base64::base64_encode(&key),
        )
        .unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    (dir, server)
}
fn device() -> Device {
    Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: crate::features::chat::enums::DeviceType::Phone,
        ips: vec![],
        aware_supported: false,
    }
}
fn target(id: &str) -> Target {
    Target {
        device_id: id.into(),
        device_name: id.into(),
        device_ip: "spoofed".into(),
        device_port: 2443,
        device_public_key: plain_libs::utils::base64::base64_encode(
            &plain_libs::crypto::ed25519_generate().1,
        ),
    }
}
async fn idle(state: &CoreRuntime) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if state.ble_pairing.0.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn ble_initiator_signs_filters_notifications_commits_shared_key_and_closes_once() {
    let (_d, local) = fixture("local");
    let (_r, remote) = fixture("remote");
    let state = local.runtime_state();
    let other = remote.runtime_state();
    let mut events = state.events.subscribe();
    let (generation, mut effects) = state.host.connect();
    let host = state.host.clone();
    let remote_state = other.clone();
    let local_state = state.clone();
    let driver = tokio::spawn(async move {
        let mut body = None;
        let mut step = 0;
        let mut notifications = std::collections::VecDeque::new();
        while let Some(effect) = effects.recv().await {
            let result = match effect["method"].as_str().unwrap() {
                "blePairConnect" => {
                    assert_eq!(effect["params"]["clientId"], "remote");
                    assert!(effect["params"].get("id").is_none());
                    assert_eq!(effect["params"].as_object().unwrap().len(), 2);
                    json!(true)
                }
                "blePairSend" => {
                    let packet = plain_libs::utils::base64::base64_decode(
                        effect["params"]["packet"].as_str().unwrap(),
                    );
                    let (metadata, body_bytes) =
                        crate::infrastructure::protocols::ble::parts(&packet).unwrap();
                    match Message::parse(std::str::from_utf8(body_bytes).unwrap()).unwrap() {
                        Message::PairRequest(payload) => {
                            assert_eq!(metadata, b"local");
                            let request = crate::features::peers::pairing::protocol::PairingRequest {
                                to_id: "remote".into(),
                                target_public_key: plain_libs::utils::base64::base64_encode(
                                    &crate::features::services::peers::peer_wire::signing_keypair(&remote_state.prefs).unwrap()[32..],
                                ),
                                from_id: "local".into(),
                                from_ip: String::new(),
                                payload,
                            };
                            assert!(crate::features::peers::pairing::security::verify_request(
                                &request
                            ));
                            body = Some(request.clone());
                            pairing::receive_request(&remote_state.pairing, &request).unwrap();
                            let incoming = remote_state
                                .pairing
                                .incoming_target(&request.from_id, &request.payload.signature)
                                .unwrap();
                            remote_state
                                .pairing_runtime
                                .confirmations
                                .register(&remote_state, &request, incoming)
                                .unwrap();
                            let response = remote_state
                                .pairing_runtime
                                .confirmations
                                .respond(&remote_state, &request, true, device())
                                .unwrap();
                            notifications.push_back("malformed".into());
                            let mut wrong = response.clone();
                            wrong.request_signature = "unrelated".into();
                            notifications.push_back(Message::PairResponse(wrong).wire().unwrap());
                            notifications
                                .push_back(Message::PairResponse(response).wire().unwrap());
                        }
                        Message::PairVerification(verification) => {
                            let super::super::pairing_confirmation::Action::Notify {
                                message, ..
                            } = remote_state
                                .pairing_runtime
                                .confirmations
                                .process(&remote_state, verification)
                                .unwrap()
                            else {
                                panic!()
                            };
                            notifications.push_back(message.wire().unwrap());
                        }
                        _ => panic!("Unexpected pairing wire message"),
                    }
                    json!(true)
                }
                "blePairWait" => {
                    assert_eq!(local_state.pairing.states()[0].1, "PAIRING");
                    assert!(effect["params"]["timeoutMs"].as_u64().unwrap() <= 1000);
                    step += 1;
                    if notifications.is_empty() {
                        let ticket = local_state.pairing.tickets()[0].clone();
                        if let Ok(super::super::pairing_confirmation::Action::Send {
                            message: Message::PairVerification(finish),
                            ..
                        }) = local_state
                            .pairing_runtime
                            .confirmations
                            .confirm(&local_state, &ticket.generation)
                        {
                            let super::super::pairing_confirmation::Action::Notify {
                                message, ..
                            } = remote_state
                                .pairing_runtime
                                .confirmations
                                .process(&remote_state, finish)
                                .unwrap()
                            else {
                                panic!()
                            };
                            notifications.push_back(message.wire().unwrap());
                        }
                    }
                    let notification = notifications.pop_front().unwrap();
                    json!({"connected":true,"notification":notification})
                }
                "blePairClose" => {
                    assert!(body.is_some());
                    assert_eq!(step, 4);
                    host.reply(generation, json!({"id":effect["id"],"result":true}))
                        .unwrap();
                    break;
                }
                method => panic!("Unexpected method {method}"),
            };
            host.reply(generation, json!({"id":effect["id"],"result":result}))
                .unwrap();
        }
    });
    let mut remote_target = target("remote");
    remote_target.device_public_key = plain_libs::utils::base64::base64_encode(
        &crate::features::services::peers::peer_wire::signing_keypair(&other.prefs).unwrap()[32..],
    );
    let first = start(&state, remote_target.clone(), device(), None).unwrap();
    assert_eq!(state.pairing.states()[0].1, "STARTING");
    let duplicate = start(&state, remote_target, device(), None).unwrap();
    assert_eq!(
        first["ticket"]["generation"],
        duplicate["ticket"]["generation"]
    );
    assert_eq!(first["ticket"]["deviceIp"], "");
    tokio::time::timeout(Duration::from_secs(3), driver)
        .await
        .unwrap()
        .unwrap();
    idle(&state).await;
    let local_peer = crate::infrastructure::db::chat_store::peers::get(&state.db, "remote")
        .unwrap()
        .unwrap();
    let remote_peer = crate::infrastructure::db::chat_store::peers::get(&other.db, "local")
        .unwrap()
        .unwrap();
    assert_eq!(local_peer.key, remote_peer.key);
    assert!(local_peer.is_paired());
    assert!(state.pairing.tickets().is_empty());
    assert_eq!(events.recv().await.unwrap().event_type, WS_PAIRING_STARTED);
    assert_eq!(
        events.recv().await.unwrap().event_type,
        crate::features::chat::events::WS_PAIRING_SUCCESS
    );
    remote.shutdown().await;
    local.shutdown().await;
}
#[tokio::test]
async fn cancellation_interrupts_pending_connect_and_preserves_replacement_generation() {
    let (_dir, server) = fixture("local");
    let state = server.runtime_state();
    let (generation, mut effects) = state.host.connect();
    let old = start(&state, target("remote"), device(), None).unwrap();
    let connect = effects.recv().await.unwrap();
    assert_eq!(connect["method"], "blePairConnect");
    let replacement =
        pairing::start(&state.prefs, &state.pairing, target("remote"), device()).unwrap();
    let close = tokio::time::timeout(Duration::from_secs(2), effects.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(close["method"], "blePairClose");
    assert_eq!(close["params"]["generation"], old["ticket"]["generation"]);
    state
        .host
        .reply(generation, json!({"id":connect["id"],"result":true}))
        .unwrap();
    state
        .host
        .reply(generation, json!({"id":close["id"],"result":true}))
        .unwrap();
    idle(&state).await;
    assert_eq!(
        state.pairing.tickets()[0].generation,
        replacement["ticket"]["generation"].as_str().unwrap()
    );
    assert!(
        crate::infrastructure::db::chat_store::peers::get(&state.db, "remote")
            .unwrap()
            .is_none()
    );
    assert!(
        state
            .pairing_runtime
            .confirmations
            .confirm(&state, old["ticket"]["generation"].as_str().unwrap())
            .is_err()
    );
    assert_eq!(
        state.pairing.tickets()[0].generation,
        replacement["ticket"]["generation"].as_str().unwrap()
    );
    server.shutdown().await;
}
#[tokio::test]
async fn capacity_and_actual_connect_refusal_release_only_their_ticket() {
    let (_dir, server) = fixture("local");
    let state = server.runtime_state();
    let (generation, mut effects) = state.host.connect();
    start(&state, target("first"), device(), None).unwrap();
    start(&state, target("second"), device(), None).unwrap();
    assert!(
        start(&state, target("third"), device(), None)
            .unwrap()
            .is_null()
    );
    assert_eq!(state.pairing.tickets().len(), 2);
    for _ in 0..4 {
        let effect = tokio::time::timeout(Duration::from_secs(2), effects.recv())
            .await
            .unwrap()
            .unwrap();
        let result = match effect["method"].as_str().unwrap() {
            "blePairConnect" => false,
            "blePairClose" => true,
            _ => panic!(),
        };
        state
            .host
            .reply(generation, json!({"id":effect["id"],"result":result}))
            .unwrap();
    }
    idle(&state).await;
    assert!(state.pairing.tickets().is_empty());
    assert!(
        crate::infrastructure::db::chat_store::peers::all(&state.db)
            .unwrap()
            .is_empty()
    );
    server.shutdown().await;
}
