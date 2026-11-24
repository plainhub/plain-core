use super::*;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::transport::contract::{
    CREATE_CHAT_ITEM, PeerTransport, chat_item_request,
};
use crate::infrastructure::transport::graphql_request::SignedEnvelope;
use plain_libs::serde_json::Value;
use plain_libs::serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn host_transport_preserves_signed_wire_and_requires_an_actual_receipt() {
    const ACTOR: &str = "706c6169-1ac2-825a-8c79-0951ec181d4a";
    const CHANNEL: &str = "channel";
    const CONTENT: &str = r#"{"type":"MESSAGE","value":{"text":"Fixture message"}}"#;
    const RECIPIENT_KEY: [u8; 32] = [7; 32];

    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", ACTOR).unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let signing_key =
        crate::features::prefs::security::signing_keypair(&state.prefs).unwrap();
    let public_key = plain_libs::utils::base64::base64_encode(&signing_key[32..]);
    let host = state.host.clone();
    let (generation, mut receiver) = crate::test_support::HostRequests::connect(&host);
    let transport = Transport {
        state: state.clone(),
        attempt: crate::infrastructure::transport::router::Attempt::Automatic,
    };
    let mut peer = DPeer::new(
        "fixture",
        "fixture",
        "",
        Some(443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    peer.encryption_public_key = Some(plain_libs::crypto::hpke::public_key(&RECIPIENT_KEY));
    peer.status = crate::features::chat::enums::PeerStatus::Channel;
    crate::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[peer.clone()],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let responder = {
        let host = host.clone();
        let ble = state.ble_transport.clone();
        tokio::spawn(async move {
            for (receipt, errors) in [
                (json!([]), Value::Null),
                (Value::Null, Value::Null),
                (json!([]), json!([{"message":"REJECTED"}])),
            ] {
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
                assert!(matches!(
                    ble.request_for_test(&request["params"]),
                    crate::infrastructure::protocols::ble::Request::PeerGraphql {
                        client_id, channel_id, ..
                    } if client_id == ACTOR && channel_id == CHANNEL
                ));
                let crate::infrastructure::protocols::ble::Request::PeerGraphql { body, .. } =
                    ble.request_for_test(&request["params"]) else { panic!("Expected peer request") };
                let (wire, response_key) = crate::features::services::peers::ble_transport::test_channel_wire(
                    &body, &RECIPIENT_KEY, CHANNEL, ACTOR, "fixture", &public_key,
                );
                let envelope: SignedEnvelope = plain_libs::serde_json::from_str(&wire).unwrap();
                assert_eq!(envelope.query, CREATE_CHAT_ITEM);
                assert_eq!(envelope.variables["content"], CONTENT);
                let reply = crate::features::services::peers::ble_transport::test_reply(
                    &ble,
                    &request["params"],
                    json!({"data":{"createChatItem":receipt},"errors":errors}),
                    &response_key,
                );
                host.reply(generation, json!({"id":request["id"],"result":reply}))
                    .unwrap();
            }
        })
    };
    let key = peer.encryption_public_key.unwrap();
    let dispatch = PeerDispatch { peer: &peer, client_id: ACTOR, channel_id: CHANNEL, key: &key, signing_key: &signing_key, message_id: Some("message") };
    let send = || {
        transport.queued_message(&dispatch, || {
            chat_item_request(&signing_key, CONTENT, CHANNEL)
        })
    };
    send().await.unwrap();
    assert_eq!(
        send().await.unwrap_err(),
        "Missing createChatItem delivery receipt"
    );
    assert!(send().await.unwrap_err().contains("REJECTED"));
    responder.await.unwrap();
    host.disconnect(generation);
}
