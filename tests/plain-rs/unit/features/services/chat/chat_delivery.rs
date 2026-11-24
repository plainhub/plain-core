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
    const CHANNEL_KEY: [u8; 32] = [7; 32];

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
        crate::features::services::peers::peer_wire::signing_keypair(&state.prefs).unwrap();
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
                let wire = crate::features::services::peers::ble_transport::test_peer_wire(
                    &ble,
                    &request["params"],
                    &CHANNEL_KEY,
                );
                let envelope: SignedEnvelope = plain_libs::serde_json::from_str(&wire).unwrap();
                assert_eq!(envelope.query, CREATE_CHAT_ITEM);
                assert_eq!(envelope.variables["content"], CONTENT);
                assert!(plain_libs::crypto::ed25519_verify(
                    &public_key,
                    &envelope.signing_bytes(CHANNEL),
                    &envelope.signature,
                ));
                let reply = crate::features::services::peers::ble_transport::test_reply(
                    &ble,
                    &request["params"],
                    json!({"data":{"createChatItem":receipt},"errors":errors}),
                    &CHANNEL_KEY,
                );
                host.reply(generation, json!({"id":request["id"],"result":reply}))
                    .unwrap();
            }
        })
    };
    let send = || {
        transport.queued_message(&peer, ACTOR, CHANNEL, &CHANNEL_KEY, || {
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
