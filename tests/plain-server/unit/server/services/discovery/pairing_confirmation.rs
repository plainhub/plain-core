use plain_libs::utils::base64::base64_encode;
use plain_rs::features::services::discovery::pairing;
use plain_rs::features::services::discovery::pairing_attempts::Mode;
use plain_rs::features::services::discovery::pairing_attempts::PairingPhase;
use std::sync::Arc;

fn fixture(id: &str) -> (plain_libs::temporary::TempDir, crate::server::ContentServer) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("system.json")).unwrap(),
    );
    let (key, _) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set("signature_key_pair", base64_encode(&key))
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
    }
}
#[tokio::test]
async fn scanner_sends_only_the_response_and_both_tls_peers_save_the_same_key() {
    use plain_rs::features::services::discovery::{pairing_attempts, pairing_qr};
    let (_local_dir, local_server) = fixture("display");
    let (_remote_dir, remote_server) = fixture("scanner");
    let mut ports = Vec::new();
    for server in [&local_server, &remote_server] {
        let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
        let (_, port) = server.start_public(0, 0, cert_pem, cert_key).await.unwrap();
        ports.push(port);
    }
    let local = local_server.runtime_state().core;
    let remote = remote_server.runtime_state().core;
    assert!(
        pairing_attempts::start(
            &remote,
            Some("display".into()),
            None,
            Some(Mode::Chat),
            None,
            None,
            None,
        )
        .await
        .is_err()
    );
    let mut advertised = device();
    advertised.port = ports[0];
    advertised.ips.push("127.0.0.1".into());
    let qr = pairing_qr::create_display(&local, "display".into(), advertised).unwrap();
    let mut incoming = local.events.subscribe();
    let read_peer = |state: &plain_rs::runtime::Runtime, id| {
        plain_rs::infrastructure::db::chat_store::peers::get(&state.db, id)
            .unwrap()
            .unwrap()
    };
    let signing =
        plain_rs::features::services::peers::peer_wire::signing_keypair(&remote.prefs).unwrap();
    let mut forged = plain_rs::features::peers::pairing::protocol::PairingResponse {
        request_id: qr.request_id.clone(),
        signature_public_key: Some(base64_encode(&signing[32..])),
        from_name: Some("Unverified scanner".into()),
        port: ports[1],
        device_type: device().device_type,
        ecdh_public_key: base64_encode(
            &plain_libs::crypto::EcdhSession::generate().public_key_bytes,
        ),
        accepted: true,
        ips: vec![],
        signature: String::new(),
        aware_supported: false,
    };
    forged.signature =
        plain_libs::crypto::ed25519_sign(&signing, forged.signature_data().as_bytes());
    let message = plain_rs::features::discovery::nearby_wire::Message::PairResponse(forged);
    assert!(
        !plain_rs::features::discovery::nearby_http::send(
            "127.0.0.1",
            ports[0],
            &message,
            Some("scanner"),
        )
        .await
        .unwrap()
    );
    assert!(
        plain_rs::infrastructure::db::chat_store::peers::get(&local.db, "scanner")
            .unwrap()
            .is_none()
    );

    for _ in 0..2 {
        let attempt = pairing_attempts::start(
            &remote,
            Some("display".into()),
            Some(qr.payload.clone()),
            Some(Mode::Chat),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        let complete = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let current = remote
                    .pairing_runtime
                    .attempts
                    .get(&attempt.generation)
                    .unwrap();
                if matches!(
                    current.phase,
                    PairingPhase::Succeeded | PairingPhase::Failed
                ) {
                    break current;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            complete.phase,
            PairingPhase::Succeeded,
            "{:?}",
            complete.error
        );
        assert_eq!(
            complete.verification_code.as_deref(),
            Some(qr.verification_code.as_str())
        );
        assert_eq!(qr.verification_code.len(), 8);
        assert!(
            qr.verification_code
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        );
        assert_eq!(
            read_peer(&local, "scanner").key,
            read_peer(&remote, "display").key
        );
    }
    let mut completed = 0;
    while let Ok(event) = incoming.try_recv() {
        assert_ne!(event.event_type, "PAIRING_REQUEST_RECEIVED");
        if event.event_type == "PAIRING_SUCCESS" {
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
