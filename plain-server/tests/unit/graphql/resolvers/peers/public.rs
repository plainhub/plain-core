use super::*;
use crate::assembly::server::ServerState;
use crate::host::bridge::Host;

struct PublicSchema {
    state: ServerState,
    _server: crate::assembly::server::ContentServer,
}
use plain_rs::prefs::Prefs;
use serde_json::{Value, json};
use std::sync::Arc;

fn stub(host: Arc<Host>, handler: impl Fn(&str, Value) -> Value + Send + 'static) {
    let (generation, mut requests) = host.connect();
    let host = host.clone();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let Some(id) = request["id"].as_u64() else {
                continue;
            };
            let result = handler(
                &request["method"].as_str().unwrap_or_default(),
                request["params"].clone(),
            );
            let _ = host.reply(generation, json!({ "id": id, "result": result }));
        }
    });
}

fn fixture_with(
    handler: impl Fn(&str, Value) -> Value + Send + 'static,
) -> (tempfile::TempDir, PublicSchema) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    prefs
        .set("api_permissions", vec!["READ_PHONE_NUMBERS"])
        .unwrap();
    let server = crate::assembly::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let mut peer = plain_rs::db::DPeer::new(
        "peer-1",
        "Pixel",
        "192.168.1.20",
        Some(1234),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    peer.status = plain_rs::chat::enums::PeerStatus::Paired;
    plain_rs::db::chat_store::peers::save(
        &state.db,
        &[peer],
        plain_rs::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    state
        .peer_status
        .connections
        .hint(&state.db, "peer-1")
        .unwrap();
    stub(state.host.clone(), handler);
    (
        dir,
        PublicSchema {
            state,
            _server: server,
        },
    )
}

async fn query(schema: &PublicSchema, document: &str) -> Value {
    let bytes = crate::http::routes::graphql::run(
        &schema.state.public,
        &json!({"query":document}).to_string(),
        Some(schema.state.clone()),
    )
    .await
    .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn fixture() -> (tempfile::TempDir, PublicSchema) {
    fixture_with(|method, _| match method {
        "peerTransportCapabilities" => json!([]),
        "systemSimFacts" => json!([
            { "id": "sim-0", "label": "SIM 1", "number": "+15550100", "subscriptionId": 1 },
        ]),
        _other => panic!("unexpected host call {_other}"),
    })
}

#[tokio::test]
async fn a_peer_reports_its_liveness_and_kind() {
    let (_dir, schema) = fixture();
    let listed = query(
        &schema,
        r#"query { peers { id name ip status port deviceType createdAt updatedAt online } }"#,
    )
    .await;
    let rows = listed["data"]["peers"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{listed}");
    assert_eq!(rows[0]["status"], "PAIRED");
    assert_eq!(rows[0]["deviceType"], "PHONE");
    assert_eq!(rows[0]["online"], true);
    assert_eq!(rows[0]["port"], 1234);
}

/// A device with no telephony has no SIMs; an empty list is the answer the
/// contract expects, not an error the settings screen would have to catch.
#[tokio::test]
async fn sims_carry_the_slot_the_sender_chooses() {
    let (_dir, schema) = fixture();
    let listed = query(
        &schema,
        r#"query { sims { id label number subscriptionId } }"#,
    )
    .await;
    let rows = listed["data"]["sims"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{listed}");
    assert_eq!(rows[0]["subscriptionId"], 1);
    assert_eq!(rows[0]["number"], "+15550100");
}

#[tokio::test]
async fn an_empty_sim_list_is_not_an_error() {
    let (_dir, schema) = fixture_with(|method, _| match method {
        "systemSimFacts" => json!([]),
        _other => panic!("unexpected host call {_other}"),
    });
    let listed = query(&schema, r#"query { sims { id } }"#).await;
    assert!(
        listed["errors"].as_array().is_none_or(Vec::is_empty),
        "{listed}"
    );
    assert_eq!(listed["data"]["sims"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn pairing_sends_the_device_the_contract_described() {
    let (_dir, schema) = fixture();
    let paired = query(
        &schema,
        r#"mutation { pairDevice(input: { id: "peer-9" }) { deviceId generation mode phase } }"#,
    )
    .await;
    assert_eq!(
        paired["data"]["pairDevice"]["deviceId"], "peer-9",
        "{paired}"
    );
    assert_eq!(paired["data"]["pairDevice"]["mode"], "CHAT");
}

#[tokio::test]
async fn responding_requires_a_verified_current_pairing_request() {
    let (_dir, schema) = fixture();
    let responded = query(
        &schema,
        r#"mutation { respondToPairing(input: {
            fromId: "peer-9", fromName: "Tablet", port: 4321, deviceType: TABLET,
            ecdhPublicKey: "ecdh", signaturePublicKey: "sig", timestamp: 1767322,
            ips: ["192.0.2.5"], signature: "signed", fromIp: "192.0.2.5",
            awareSupported: true
        }, accepted: true) }"#,
    )
    .await;
    assert!(
        responded["errors"]
            .to_string()
            .contains("Pairing request expired or no longer available"),
        "{responded}"
    );
}

#[tokio::test]
async fn cancelling_and_unpairing_both_acknowledge() {
    let (_dir, schema) = fixture();
    for (document, field) in [
        (
            r#"mutation { cancelPairing(generation: "peer-9") }"#,
            "cancelPairing",
        ),
        (r#"mutation { unpairPeer(id: "peer-1") }"#, "unpairPeer"),
        (r#"mutation { deletePeer(id: "peer-1") }"#, "deletePeer"),
    ] {
        let result = query(&schema, document).await;
        assert!(
            result["errors"].as_array().is_none_or(Vec::is_empty),
            "{result}"
        );
        assert_eq!(result["data"][field], field != "cancelPairing", "{result}");
    }
}

#[tokio::test]
async fn unsupported_transport_does_not_call_native_pairing() {
    let (_dir, schema) = fixture();
    let mut events = schema.state.events.subscribe();
    let paired = query(
        &schema,
        r#"mutation { pairDevice(input: { id: "peer-9" }) { deviceId generation mode phase } }"#,
    )
    .await;
    assert!(
        paired["errors"].as_array().is_none_or(Vec::is_empty),
        "{paired}"
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = events.recv().await.unwrap();
            if event.event_type == "PAIRING_STATE_CHANGED" {
                let value: Value = serde_json::from_str(&event.payload).unwrap();
                if value["phase"] == "FAILED" {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn pairing_rejects_caller_supplied_discovery_metadata() {
    let (_dir, schema) = fixture();
    let result = query(
        &schema,
        r#"mutation { pairDevice(input: { id: "peer-invalid", lastSeen: "invalid-time" }) { generation } }"#,
    )
    .await;
    assert!(
        result["errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()),
        "{result}"
    );
    assert!(
        result["errors"].to_string().contains("lastSeen"),
        "{result}"
    );
}

#[tokio::test]
async fn main_identity_rename_and_pairing_code_use_shared_preferences_and_refuse_stopped_core() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let executor = core.native_graphql().unwrap();
    let identity = "{app{clientId deviceName}}";
    let response = executor
        .execute(async_graphql::Request::new(identity))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let value = response.data.into_json().unwrap();
    assert_eq!(value["app"]["clientId"], executor.client_id());
    let settings = core.native_management().unwrap().preferences().unwrap();
    assert_eq!(
        plain_rs::utils::base64::base64_decode(settings["signature_public_key"].as_str().unwrap())
            .len(),
        32
    );
    assert!(!settings["mdns_hostname"].as_str().unwrap().is_empty());
    let public_key = settings["signature_public_key"].clone();
    let changed = executor
        .execute(async_graphql::Request::new(
            r#"mutation{updateDeviceName(name:"Shared device")}"#,
        ))
        .await;
    assert!(changed.errors.is_empty(), "{:?}", changed.errors);
    let response = executor
        .execute(async_graphql::Request::new(identity))
        .await;
    let value = response.data.into_json().unwrap();
    assert_eq!(value["app"]["deviceName"], "Shared device");
    assert_eq!(
        core.native_management().unwrap().preferences().unwrap()["signature_public_key"],
        public_key
    );
    let management = core.native_management().unwrap();
    assert!(
        management.pairing_code().await.is_err(),
        "An inactive listener cannot produce a usable QR code"
    );
    assert!(
        !management
            .is_pairing_request_current("absent".into(), "stale".into())
            .unwrap()
    );
    core.stop_core().await;
    for document in [
        identity,
        r#"mutation{updateDeviceName(name:"Stopped device")}"#,
    ] {
        assert!(
            !executor
                .execute(async_graphql::Request::new(document))
                .await
                .errors
                .is_empty()
        );
    }
}
