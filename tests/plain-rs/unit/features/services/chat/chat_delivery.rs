use super::*;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::transport::contract::PeerTransport;
use plain_libs::serde_json::Value;
use plain_libs::serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn host_transport_preserves_signed_wire_and_requires_an_actual_receipt() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "self").unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
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
    peer.key = plain_libs::utils::base64::base64_encode(&[7; 32]);
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
            for (index, data) in [
                json!({"createChatItem":[]}),
                json!({"createChatItem":null}),
                json!({"createChatItem":[]}),
            ]
            .into_iter()
            .enumerate()
            {
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
                    matches!(ble.request_for_test(&request["params"]), crate::infrastructure::protocols::ble::Request::PeerGraphql { channel_id, .. } if channel_id == "channel")
                );
                assert_eq!(
                    crate::features::services::peers::ble_transport::test_peer_wire(
                        &ble,
                        &request["params"],
                        &[7; 32]
                    ),
                    "signed|123|request"
                );
                let errors = if data == json!({"createChatItem":null}) {
                    Value::Null
                } else if data == json!({"createChatItem":[]}) && index == 2 {
                    json!([{"message":"rejected"}])
                } else {
                    Value::Null
                };
                host.reply(
                    generation,
                    json!({"id":request["id"],"result":crate::features::services::peers::ble_transport::test_reply(&ble, &request["params"], json!({"data":data,"errors":errors}),&[7;32])}),
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
