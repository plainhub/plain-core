use plain_libs::utils::base64::base64_encode;
use plain_rs::features::services::discovery::pairing;
use plain_rs::features::services::discovery::pairing_attempts::Mode;
use std::sync::Arc;

fn fixture(id: &str) -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("system.json")).unwrap(),
    );
    let (key, _) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set("signature_private_key", base64_encode(&key[..32]))
        .unwrap();
    let server =
        crate::server::ContentServer::start(directory.path(), &base64_encode(&[3; 32]), prefs)
            .unwrap();
    (directory, server)
}
fn device() -> pairing::Device {
    pairing::Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: plain_rs::features::chat::enums::DeviceType::Phone,
        ips: vec![],
        aware_supported: false,
        remote_access: false,
    }
}
#[tokio::test]
async fn scanner_sends_only_the_response_and_both_tls_peers_save_the_same_key() {
    use plain_rs::features::services::discovery::{pairing_attempts, pairing_qr};
    let (_local_dir, local_server) = fixture("706c6169-f9bd-8908-9bd5-f02ea4f40dea");
    let (_remote_dir, remote_server) = fixture("706c6169-a13e-8ac0-9d42-e0221c9fec7e");
    let mut ports = Vec::new();
    for server in [&local_server, &remote_server] {
        let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
        let (_, port) = server.start_public(0, 0, cert_pem, cert_key).await.unwrap();
        ports.push(port);
    }
    let local = local_server.runtime_state().core;
    let remote = remote_server.runtime_state().core;
    assert!(
        pairing_attempts::start(&remote, "706c6169-f9bd-8908-9bd5-f02ea4f40dea".into(), Mode::Chat, None, None)
        .await
        .is_err()
    );
    let mut advertised = device();
    advertised.port = ports[0];
    advertised.ips.push("127.0.0.1".into());
    let qr = pairing_qr::create_display(&local, "706c6169-f9bd-8908-9bd5-f02ea4f40dea".into(), advertised).unwrap();
    let mut incoming = local.events.subscribe();
    let read_peer = |state: &plain_rs::runtime::Runtime, id| {
        plain_rs::infrastructure::db::chat_store::peers::get(&state.db, id)
            .unwrap()
            .unwrap()
    };
    let signing =
        plain_rs::features::prefs::security::signing_keypair(&remote.prefs).unwrap();
    let mut forged = plain_rs::features::peers::pairing::protocol::PairingAccept {
        encryption_public_key: plain_libs::utils::base64::base64_encode(&[41; 32]),
        request_id: qr.request_id.clone(),
        signature_public_key: Some(base64_encode(&signing[32..])),
        from_name: Some("Unverified scanner".into()),
        port: ports[1],
        device_type: device().device_type,
        ecdh_public_key: base64_encode(
            &plain_libs::crypto::EcdhSession::generate().public_key_bytes,
        ),
        ips: vec![],
        signature: String::new(),
        aware_supported: false,
    };
    forged.signature =
        plain_libs::crypto::ed25519_sign(&signing, forged.signature_data().as_bytes());
    let message = plain_rs::features::discovery::nearby_wire::Message::PairingAccept(forged);
    assert!(
        !plain_rs::features::discovery::nearby_http::send(
            "127.0.0.1",
            ports[0],
            &message,
            Some("706c6169-a13e-8ac0-9d42-e0221c9fec7e"),
        )
        .await
        .unwrap()
    );
    assert!(
        plain_rs::infrastructure::db::chat_store::peers::get(&local.db, "706c6169-a13e-8ac0-9d42-e0221c9fec7e")
            .unwrap()
            .is_none()
    );

    let native = remote_server.native_graphql().unwrap();
    let parsed = native.execute(plain_graphql::Request::new("query($code:String!){parsePairingQrCode(code:$code){id}}")
        .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"code":qr.payload})))).await;
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert_eq!(parsed.data.into_json().unwrap()["parsePairingQrCode"]["id"], "706c6169-f9bd-8908-9bd5-f02ea4f40dea");
    for _ in 0..2 {
        let mut notices = remote.events.subscribe();
        let response = native.execute(plain_graphql::Request::new(r#"mutation{pairPeer(peerId:"706c6169-f9bd-8908-9bd5-f02ea4f40dea",input:{ mode:CHAT})}"#)).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().unwrap()["pairPeer"], true);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Ok(event) = notices.recv().await {
                assert_ne!(event.event_type, "PAIRING_FAILED", "{}", event.payload);
                if event.event_type == "PAIRING_COMPLETED" { return; }
            }
            panic!("pairing notification closed");
        }).await.unwrap();
        assert_eq!(qr.verification_code.len(), 8);
        assert!(
            qr.verification_code
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        );
        assert_eq!(
            read_peer(&local, "706c6169-a13e-8ac0-9d42-e0221c9fec7e").key,
            read_peer(&remote, "706c6169-f9bd-8908-9bd5-f02ea4f40dea").key
        );
    }
    let mut completed = 0;
    while let Ok(event) = incoming.try_recv() {
        assert_ne!(event.event_type, "PAIRING_REQUESTED");
        if event.event_type == "PAIRING_COMPLETED" {
            let payload: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&event.payload).unwrap();
            assert_eq!(payload["requestId"], qr.request_id);
            assert_eq!(payload["ip"], "127.0.0.1");
            assert_eq!(payload["port"], ports[1]);
            completed += 1;
        }
    }
    assert_eq!(completed, 1);
    local_server.shutdown().await;
    remote_server.shutdown().await;
}
